use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread, usize,
};

use cpal::{
    Device, Sample, Stream, StreamConfig,
    traits::{DeviceTrait, StreamTrait},
};
use nnnoiseless::DenoiseState;
use ringbuf::{
    HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use tracing::{error, info};

const TRACING_TARGET: &str = "app";

type P = ringbuf::wrap::caching::Caching<
    Arc<ringbuf::SharedRb<ringbuf::storage::Heap<f32>>>,
    true,
    false,
>;
type C = ringbuf::wrap::caching::Caching<
    Arc<ringbuf::SharedRb<ringbuf::storage::Heap<f32>>>,
    false,
    true,
>;

#[allow(dead_code)]
pub struct SelfListen {
    input_stream: Stream,
    output_stream: Stream,
    denoise_thread_run: Arc<AtomicBool>,
}

impl SelfListen {
    pub fn new(input_device: &Device, output_device: &Device) -> Self {
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

        let (input_producer, input_consumer) = HeapRb::<f32>::new(4096).split();
        let (denoise_producer, denoise_consumer) = HeapRb::<f32>::new(4096).split();

        let input_stream = Self::create_input_stream(input_device, &input_config, input_producer);
        let denoise_thread_run =
            Self::create_denoise_thread(input_config.channels, input_consumer, denoise_producer);
        let output_stream =
            Self::create_output_stream(output_device, &output_config, denoise_consumer);

        input_stream.play().expect("Failed to play input stream");
        output_stream.play().expect("Failed to play output stream");

        SelfListen {
            input_stream,
            output_stream,
            denoise_thread_run,
        }
    }

    fn deinterleave(channels: usize, input: &Vec<f32>, output: &mut Vec<Vec<f32>>) {
        for (i, val) in input.iter().enumerate() {
            output[i % channels][i / channels] = *val;
        }
    }

    fn interleave(input: &Vec<Vec<f32>>, output: &mut Vec<f32>) {
        for i in 0..input[0].len() {
            for channel in 0..input.len() {
                output[input.len() * i + channel] = input[channel][i];
            }
        }
    }

    fn create_input_stream(
        input_device: &Device,
        input_config: &StreamConfig,
        mut input_producer: P,
    ) -> Stream {
        input_device
            .build_input_stream(
                input_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // `data` is slice [channel_0_sample_0, channel_1_sample_0, channel_0_sample_1, channel_1_sample_1 ...]
                    for sample in data {
                        input_producer
                            .try_push(*sample)
                            .expect("Failed to push to input buffer");
                    }
                },
                |err| error!(target: TRACING_TARGET, "An error occurred on input stream: {err}"),
                None,
            )
            .expect("Failed to build input stream")
    }

    fn create_output_stream(
        output_device: &Device,
        output_config: &StreamConfig,
        mut denoise_consumer: C,
    ) -> Stream {
        output_device
            .build_output_stream(
                output_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if denoise_consumer.occupied_len() >= data.len() {
                        for sample in data.iter_mut() {
                            *sample = denoise_consumer
                                .try_pop()
                                .expect("Failed to pop from denoise buffer");
                        }
                    }
                },
                |err| error!(target: TRACING_TARGET, "An error occurred on input stream: {err}"),
                None,
            )
            .expect("Failed to build output stream")
    }

    fn create_denoise_thread(
        channels: u16,
        mut input_consumer: C,
        mut denoise_producer: P,
    ) -> Arc<AtomicBool> {
        info!(target: TRACING_TARGET, "Starting denoise thread");

        let denoise_thread_run: Arc<AtomicBool> = Arc::new(true.into());
        let thread_run: Arc<AtomicBool> = denoise_thread_run.clone();

        thread::spawn(move || {
            let channels = channels as usize;

            let mut denoise: Vec<Box<DenoiseState>> = vec![DenoiseState::new(); channels];
            let mut denoise_buffer: Vec<f32> =
                vec![Sample::EQUILIBRIUM; DenoiseState::FRAME_SIZE * channels];
            let mut denoise_process_buffer: Vec<f32> =
                vec![Sample::EQUILIBRIUM; DenoiseState::FRAME_SIZE];
            let mut denoise_first: bool = true;
            let mut deinterleaved_buffer: Vec<Vec<f32>> =
                vec![vec![Sample::EQUILIBRIUM; DenoiseState::FRAME_SIZE]; channels];

            while thread_run.load(Ordering::Relaxed) {
                if input_consumer.occupied_len() >= DenoiseState::FRAME_SIZE * channels {
                    for sample in denoise_buffer.iter_mut() {
                        *sample = input_consumer
                            .try_pop()
                            .expect("Failed to pop from input buffer")
                    }

                    Self::deinterleave(channels, &denoise_buffer, &mut deinterleaved_buffer);

                    for i in 0..channels {
                        denoise[i]
                            .process_frame(&mut denoise_process_buffer, &deinterleaved_buffer[i]);
                        deinterleaved_buffer[i] = denoise_process_buffer.clone();
                    }

                    Self::interleave(&deinterleaved_buffer, &mut denoise_buffer);

                    if denoise_first {
                        denoise_first = false;
                    } else {
                        for sample in denoise_buffer.iter() {
                            denoise_producer
                                .try_push(*sample)
                                .expect("Failed to push to denoise buffer");
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }

            info!(target: TRACING_TARGET, "Stopping denoise thread");
        });
        denoise_thread_run
    }
}

impl Drop for SelfListen {
    fn drop(&mut self) {
        self.denoise_thread_run.store(false, Ordering::Relaxed);
    }
}
