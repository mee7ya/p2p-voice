use std::{
    mem::take, rc::Rc, sync::{
        atomic::{AtomicBool, Ordering}, Arc
    }, thread::{self, sleep}, time::Duration, usize
};

use cpal::{
    Device, Sample, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
use nnnoiseless::DenoiseState;
use ringbuf::{
    HeapRb,
    traits::{Consumer, Observer, Producer, Split, SplitRef},
};
use rubato::{FftFixedOut, Resampler};
use tracing::{error, info};

const TRACING_TARGET: &str = "app";

type P = ringbuf::wrap::caching::Caching<
    Arc<ringbuf::SharedRb<ringbuf::storage::Heap<f32>>>,
    true,
    false,
>;
type C =
    ringbuf::wrap::caching::Caching<ringbuf::SharedRb<ringbuf::storage::Heap<f32>>, false, true>;

pub struct Playback {
    input_device: Device,
    output_device: Device,
    input_config: StreamConfig,
    output_config: StreamConfig,
    input_producer: P,
    denoise_buffer: HeapRb<f32>,
    input_stream: Option<Stream>,
}

impl Playback {
    pub fn new(input_device: &Device, output_device: &Device) -> Self {
        let input_device = input_device.clone();
        let output_device = output_device.clone();

        let input_config: StreamConfig = input_device
            .default_input_config()
            .expect("Failed to get default input config")
            .into();
        info!(target: TRACING_TARGET, "Input stream config has {} channel(s), {}Hz sample rate", input_config.channels, input_config.sample_rate.0);

        let output_config: StreamConfig = output_device
            .default_output_config()
            .expect("Failed to get default output config")
            .into();
        info!(target: TRACING_TARGET, "Output stream config has {} channel(s), {}Hz sample rate", output_config.channels, output_config.sample_rate.0);

        let (mut input_producer, mut input_consumer) = HeapRb::new(4096).split();
        let denoise_buffer: HeapRb<f32> = HeapRb::new(4096);

        Self {
            input_device: input_device.clone(),
            output_device: output_device.clone(),
            input_config,
            output_config,
            input_producer,
            denoise_buffer,
            input_stream: None,
        }
    }

    // fn create_input_stream(mut self) {
    //     self.input_stream = Some(self
    //         .input_device
    //         .build_input_stream(
    //             &self.input_config,
    //             move |data: &[f32], _: &cpal::InputCallbackInfo| {
    //                 // `data` is slice [channel_0_sample_0, channel_1_sample_0, channel_0_sample_1, channel_1_sample_1 ...]
    //                 take(self.input_producer);
    //             },
    //             |err| error!(target: TRACING_TARGET, "An error occurred on input stream: {}", err),
    //             None,
    //         )
    //         .expect("Failed to build input stream"));
    //     }

    // pub fn play(self) {
    //     self.create_input_stream();
    //     self.create_output_stream()
    // }
}

fn transform_to_resampler(input: &Vec<f32>, input_channels: usize, output: &mut Vec<Vec<f32>>) {
    let mut idx: Vec<usize> = vec![0; input_channels];
    for (i, val) in input.iter().enumerate() {
        output[i % input_channels][idx[i % input_channels]] = *val;
        idx[i % input_channels] += 1;
    }
}

fn transform_from_resampler(input: &Vec<Vec<f32>>, output: &mut Vec<f32>) {
    let mut output_idx: usize = 0;
    for i in 0..input[0].len() {
        for channel in 0..input.len() {
            output[output_idx] = input[channel][i];
            output_idx += 1;
        }
    }
}

pub fn self_listen(
    input_device: &Device,
    output_device: &Device,
) -> (Arc<AtomicBool>, Stream, Stream) {
    let input_config: StreamConfig = input_device
        .default_input_config()
        .expect("No default input config")
        .into();

    info!(target: TRACING_TARGET, "Input stream config has {} channel(s), {}Hz sample rate", input_config.channels, input_config.sample_rate.0);

    let output_config: StreamConfig = output_device
        .default_output_config()
        .expect("No default output config")
        .into();

    info!(target: TRACING_TARGET, "Output stream config has {} channel(s), {}Hz sample rate", output_config.channels, output_config.sample_rate.0);

    let input_batch: usize =
        (input_config.sample_rate.0 as f32 * 0.01_f32 * input_config.channels as f32) as usize;
    let heap_rb = HeapRb::<f32>::new(input_batch * 100);
    let (mut producer, mut consumer) = heap_rb.split();

    let heap_input_denoise = HeapRb::<f32>::new(input_batch * 100);
    let (mut input_producer, mut denoise_consumer) = heap_input_denoise.split();

    let mut denoise = DenoiseState::new();
    let mut denoise_buffer: Vec<f32> = vec![Sample::EQUILIBRIUM; DenoiseState::FRAME_SIZE];
    let mut denoise_output_buffer: Vec<f32> = denoise_buffer.clone();
    let mut denoise_first: bool = true;
    let denoise_thread_run: Arc<AtomicBool> = Arc::new(true.into());
    let denoise_thread_run_return: Arc<AtomicBool> = denoise_thread_run.clone();

    let mut resampler: Option<FftFixedOut<f32>> = None;
    let mut resampler_output_buffer: Option<Vec<Vec<f32>>> = None;
    let mut resampler_input_buffer: Option<Vec<Vec<f32>>> = None;
    let mut temp_output_buffer: Option<Vec<f32>> = None;

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        // `data` is slice [channel_0_sample_0, channel_1_sample_0, channel_0_sample_1, channel_1_sample_1 ...]
        for sample in data {
            input_producer
                .try_push(*sample)
                .expect("Failed to push into input buffer");
        }
    };

    let denoise_fn = move || {
        info!(target: TRACING_TARGET, "Started denoising thread");
        while denoise_thread_run.load(Ordering::Relaxed) {
            if denoise_consumer.occupied_len() >= DenoiseState::FRAME_SIZE {
                for sample in denoise_buffer.iter_mut() {
                    *sample = denoise_consumer
                        .try_pop()
                        .expect("Failed to pop from input buffer")
                }
                denoise.process_frame(&mut denoise_output_buffer, &denoise_buffer);

                if denoise_first {
                    denoise_first = false;
                } else {
                    for sample in denoise_output_buffer.iter() {
                        producer
                            .try_push(*sample)
                            .expect("Failed to push to output buffer");
                    }
                }
            }
        }
        info!(target: TRACING_TARGET, "Dropping denoising thread");
    };

    let mut temp_buffer: Vec<f32> = Vec::new();
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        if (temp_buffer.is_empty() && consumer.occupied_len() >= data.len())
            || (!temp_buffer.is_empty() && consumer.occupied_len() >= temp_buffer.len())
        {
            if temp_buffer.is_empty() {
                if input_config.sample_rate != output_config.sample_rate {
                    info!(target: TRACING_TARGET, "Sample rate mismatch, creating resampler");
                    resampler = Some(
                        FftFixedOut::<f32>::new(
                            input_config.sample_rate.0 as usize,
                            output_config.sample_rate.0 as usize,
                            data.len() / output_config.channels as usize,
                            1,
                            output_config.channels as usize,
                        )
                        .expect("Failed to create resampler"),
                    );

                    resampler_input_buffer =
                        Some(resampler.as_ref().unwrap().input_buffer_allocate(true));
                    info!(target: TRACING_TARGET, "Resampler input buffer has {}x{} size", resampler_input_buffer.as_ref().unwrap().len(), resampler_input_buffer.as_ref().unwrap()[0].len());

                    resampler_output_buffer =
                        Some(resampler.as_ref().unwrap().output_buffer_allocate(true));
                    info!(target: TRACING_TARGET, "Resampler output buffer has {}x{} size", resampler_output_buffer.as_ref().unwrap().len(), resampler_output_buffer.as_ref().unwrap()[0].len());

                    temp_output_buffer =
                        Some(vec![
                            Sample::EQUILIBRIUM;
                            resampler_output_buffer.as_ref().unwrap().len()
                                * resampler_output_buffer.as_ref().unwrap()[0].len()
                        ]);

                    temp_buffer = vec![
                        Sample::EQUILIBRIUM;
                        resampler.as_ref().unwrap().input_frames_max()
                            * output_config.channels as usize
                    ];
                } else {
                    temp_buffer = vec![Sample::EQUILIBRIUM; data.len()];
                }
                info!(target: TRACING_TARGET, "Temp buffer has {} size", temp_buffer.len());
                return;
            }
            consumer.pop_slice(temp_buffer.as_mut_slice());
            match resampler {
                Some(ref mut r) => {
                    // transform to [[channel_0_sample_0, channel_0_sample_1], [channel_1_sample_0, channel_1_sample_1], ...]
                    transform_to_resampler(
                        &temp_buffer,
                        input_config.channels as usize,
                        resampler_input_buffer.as_mut().unwrap(),
                    );

                    r.process_into_buffer(
                        resampler_input_buffer.as_ref().unwrap(),
                        resampler_output_buffer.as_mut().unwrap(),
                        None,
                    )
                    .expect("Failed to resample");

                    // revert transformation back for stream
                    transform_from_resampler(
                        resampler_output_buffer.as_ref().unwrap(),
                        temp_output_buffer.as_mut().unwrap(),
                    );

                    for (sample, val) in data.iter_mut().zip(temp_output_buffer.as_ref().unwrap()) {
                        *sample = *val;
                    }
                }
                None => {
                    for (sample, val) in data.iter_mut().zip(&temp_buffer) {
                        *sample = *val;
                    }
                }
            }
        }
    };

    let input_stream = input_device
        .build_input_stream(
            &input_config,
            input_data_fn,
            |err| error!(target: TRACING_TARGET, "An error occurred on input stream: {}", err),
            None,
        )
        .expect("Failed to build input stream");
    let output_stream = output_device
        .build_output_stream(
            &output_config,
            output_data_fn,
            |err| error!(target: TRACING_TARGET, "An error occurred on output stream: {}", err),
            None,
        )
        .expect("Failed to build output stream");

    thread::spawn(denoise_fn);

    input_stream.play().expect("Failed to play input stream");
    output_stream.play().expect("Failed to play output stream");

    (denoise_thread_run_return, input_stream, output_stream)
}
