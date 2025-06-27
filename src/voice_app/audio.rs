use std::{
    net::UdpSocket,
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
use opus::{Application, Channels, Decoder, Encoder};
use ringbuf::{
    HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use rubato::{FftFixedIn, Resampler};
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
    channels: usize,
    input_device: &Device,
    input_config: &StreamConfig,
    mut input_producer: P,
) -> Stream {
    input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // `data` is slice [channel_0_sample_0, channel_1_sample_0, channel_0_sample_1, channel_1_sample_1 ...]
                for sample in data.chunks(channels) {
                    if input_producer.is_full() {
                        continue;
                    }
                    input_producer
                        .try_push(sample.into_iter().sum::<f32>() / channels as f32)
                        .expect("Failed to push to input buffer");
                }
            },
            |err| error!(target: TRACING_TARGET, "An error occurred on input stream: {err}"),
            None,
        )
        .expect("Failed to build input stream")
}

fn create_output_stream(
    channels: usize,
    output_device: &Device,
    output_config: &StreamConfig,
    mut resampler_consumer: C,
) -> Stream {
    let mut resampled: f32 = Sample::EQUILIBRIUM;
    output_device
        .build_output_stream(
            output_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for (i, sample) in data.iter_mut().enumerate() {
                    if i % channels == 0 {
                        resampled = resampler_consumer.try_pop().unwrap_or(Sample::EQUILIBRIUM);
                    }
                    *sample = resampled;
                }
            },
            |err| error!(target: TRACING_TARGET, "An error occurred on input stream: {err}"),
            None,
        )
        .expect("Failed to build output stream")
}

fn create_denoise_thread(
    channels: usize,
    mut input_consumer: C,
    mut denoise_producer: P,
) -> Arc<AtomicBool> {
    info!(target: TRACING_TARGET, "Starting denoise thread");

    let denoise_thread_run: Arc<AtomicBool> = Arc::new(true.into());
    let thread_run: Arc<AtomicBool> = denoise_thread_run.clone();

    thread::spawn(move || {
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
                        * (i16::MAX as f32)
                }

                deinterleave(channels, &denoise_buffer, &mut deinterleaved_buffer);

                for i in 0..channels {
                    denoise[i].process_frame(&mut denoise_process_buffer, &deinterleaved_buffer[i]);
                    deinterleaved_buffer[i] = denoise_process_buffer.clone();
                }

                interleave(&deinterleaved_buffer, &mut denoise_buffer);

                if denoise_first {
                    denoise_first = false;
                } else {
                    for sample in denoise_buffer.iter() {
                        if denoise_producer.is_full() {
                            continue;
                        }
                        denoise_producer
                            .try_push(*sample / (i16::MAX as f32))
                            .expect("Failed to push to denoise buffer");
                    }
                }
            }
        }

        info!(target: TRACING_TARGET, "Stopping denoise thread");
    });
    denoise_thread_run
}

fn create_resampler_thread(
    channels: usize,
    input_sample_rate: usize,
    output_sample_rate: usize,
    mut input_consumer: C,
    mut resampler_producer: P,
) -> Arc<AtomicBool> {
    info!(target: TRACING_TARGET, "Starting resample thread");

    let resampler_thread_run: Arc<AtomicBool> = Arc::new(true.into());
    let thread_run: Arc<AtomicBool> = resampler_thread_run.clone();

    thread::spawn(move || {
        let resampler_chunk_size: usize = 960;
        let mut resampler = FftFixedIn::<f32>::new(
            input_sample_rate,
            output_sample_rate,
            resampler_chunk_size,
            1,
            channels,
        )
        .expect("Failed to create input buffer");

        let mut deinterleaved = resampler.input_buffer_allocate(true);
        let mut resample_process_buffer = resampler.output_buffer_allocate(true);

        let mut resampler_buffer: Vec<f32> =
            vec![Sample::EQUILIBRIUM; resampler_chunk_size * channels];
        let mut interleaved: Vec<f32> =
            vec![Sample::EQUILIBRIUM; resample_process_buffer[0].len() * channels];

        while thread_run.load(Ordering::Relaxed) {
            if input_consumer.occupied_len() >= (resampler_chunk_size * channels) {
                for sample in resampler_buffer.iter_mut() {
                    *sample = input_consumer
                        .try_pop()
                        .expect("Failed to pop from input buffer");
                }

                deinterleave(channels, &resampler_buffer, &mut deinterleaved);
                resampler
                    .process_into_buffer(&deinterleaved, &mut resample_process_buffer, None)
                    .expect("Failed to resample");
                interleave(&resample_process_buffer, &mut interleaved);

                for sample in interleaved.iter() {
                    if resampler_producer.is_full() {
                        continue;
                    }
                    resampler_producer
                        .try_push(*sample)
                        .expect("Failed to push to resampler buffer");
                }
            }
        }

        info!(target: TRACING_TARGET, "Stopping resample thread");
    });

    resampler_thread_run
}

fn create_opus_encoder_thread(
    mut consumer: C,
    sample_rate: u32,
    channels: usize,
    sender: UdpSocket,
) -> Arc<AtomicBool> {
    if channels > 2 {
        panic!("Opus doesn't support more than 2 channels");
    }

    info!(target: TRACING_TARGET, "Starting encoder thread");

    let encoder_thread_run: Arc<AtomicBool> = Arc::new(true.into());
    let thread_run: Arc<AtomicBool> = encoder_thread_run.clone();

    thread::spawn(move || {
        let mut encoder: Encoder = Encoder::new(
            sample_rate,
            if channels == 2 {
                Channels::Stereo
            } else {
                Channels::Mono
            },
            Application::Voip,
        )
        .expect("Failed to create Opus encoder");

        let mut encoder_input_buffer: Vec<f32> = vec![Sample::EQUILIBRIUM; 960];
        let mut encoder_output_buffer: Vec<u8> = vec![Sample::EQUILIBRIUM; 960];

        while thread_run.load(Ordering::Relaxed) {
            if consumer.occupied_len() >= 960 {
                for sample in encoder_input_buffer.iter_mut() {
                    *sample = consumer.try_pop().expect("Failed to pop from consumer");
                }

                let encoded = encoder
                    .encode_float(&encoder_input_buffer, &mut encoder_output_buffer)
                    .expect("Failed to encode");
                if let Err(_e) = sender.send(&encoder_output_buffer[..encoded]) {
                    continue;
                }
            }
        }

        info!(target: TRACING_TARGET, "Stopping encoder thread");
    });

    encoder_thread_run
}

fn create_opus_decoder_thread(
    mut producer: P,
    sample_rate: u32,
    channels: usize,
    receiver: UdpSocket,
) -> Arc<AtomicBool> {
    if channels > 2 {
        panic!("Opus doesn't support more than 2 channels");
    }

    info!(target: TRACING_TARGET, "Starting decoder thread");

    let decoder_thread_run: Arc<AtomicBool> = Arc::new(true.into());
    let thread_run: Arc<AtomicBool> = decoder_thread_run.clone();

    thread::spawn(move || {
        let mut decoder: Decoder = Decoder::new(
            sample_rate,
            if channels == 2 {
                Channels::Stereo
            } else {
                Channels::Mono
            },
        )
        .expect("Failed to create decoder");

        let mut decoder_input_buffer: Vec<u8> = vec![Sample::EQUILIBRIUM; 960];
        let mut decoder_output_buffer: Vec<f32> = vec![Sample::EQUILIBRIUM; 960];

        while thread_run.load(Ordering::Relaxed) {
            if let Ok(received) = receiver.recv(&mut decoder_input_buffer) {
                let decoded = decoder
                    .decode_float(
                        &decoder_input_buffer[..received],
                        &mut decoder_output_buffer,
                        false,
                    )
                    .expect("Failed to decode");
                for sample in &decoder_output_buffer[..decoded] {
                    producer
                        .try_push(*sample)
                        .expect("Failed to push to producer");
                }
            }
        }

        info!(target: TRACING_TARGET, "Stopping decoder thread");
    });

    decoder_thread_run
}

#[allow(dead_code)]
pub struct SelfListen {
    input_stream: Stream,
    output_stream: Stream,
    denoise_thread_run: Arc<AtomicBool>,
    resampler_input_thread_run: Arc<AtomicBool>,
    resampler_output_thread_run: Arc<AtomicBool>,
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

        let (input_producer, input_consumer) = HeapRb::<f32>::new(8192 * 2).split();
        let (resampler_input_producer, resampler_input_consumer) =
            HeapRb::<f32>::new(8192 * 2).split();
        let (denoise_producer, denoise_consumer) = HeapRb::<f32>::new(8192 * 2).split();
        let (resampler_output_producer, resampler_output_consumer) =
            HeapRb::<f32>::new(8192 * 2).split();

        let input_stream = create_input_stream(
            input_config.channels as usize,
            input_device,
            &input_config,
            input_producer,
        );
        let resampler_input_thread_run = create_resampler_thread(
            1,
            input_config.sample_rate.0 as usize,
            48000_usize,
            input_consumer,
            resampler_input_producer,
        );
        let denoise_thread_run =
            create_denoise_thread(1, resampler_input_consumer, denoise_producer);
        let resampler_output_thread_run = create_resampler_thread(
            1,
            48000_usize,
            output_config.sample_rate.0 as usize,
            denoise_consumer,
            resampler_output_producer,
        );
        let output_stream = create_output_stream(
            output_config.channels as usize,
            output_device,
            &output_config,
            resampler_output_consumer,
        );

        input_stream.play().expect("Failed to play input stream");
        output_stream.play().expect("Failed to play output stream");

        SelfListen {
            input_stream,
            output_stream,
            denoise_thread_run,
            resampler_input_thread_run,
            resampler_output_thread_run,
        }
    }
}

impl Drop for SelfListen {
    fn drop(&mut self) {
        self.denoise_thread_run.store(false, Ordering::Relaxed);
        self.resampler_input_thread_run
            .store(false, Ordering::Relaxed);
        self.resampler_output_thread_run
            .store(false, Ordering::Relaxed);
    }
}

#[allow(dead_code)]
pub struct P2P {
    input_stream: Stream,
    output_stream: Stream,
    resampler_input_thread_run: Arc<AtomicBool>,
    denoise_thread_run: Arc<AtomicBool>,
    encoder_thread_run: Arc<AtomicBool>,
    decoder_thread_run: Arc<AtomicBool>,
    resampler_output_thread_run: Arc<AtomicBool>,
}

impl P2P {
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

        let port: usize = 4000;

        info!(target: TRACING_TARGET, "Binding UDP socket on port {port}");
        let socket: UdpSocket =
            UdpSocket::bind(format!("0.0.0.0:{port}")).expect("Failed to bind udp socket");
        socket
            .set_nonblocking(true)
            .expect("Failed to move socket into nonblocking mode");
        socket.connect("127.0.0.1:4000").expect("Failed to connect");

        let socket_sender = socket.try_clone().expect("Failed to clone socket");

        let (input_producer, input_consumer) = HeapRb::<f32>::new(8192 * 2).split();
        let (resampler_input_producer, resampler_input_consumer) =
            HeapRb::<f32>::new(8192 * 2).split();
        let (denoise_producer, denoise_consumer) = HeapRb::<f32>::new(8192 * 2).split();
        let (decoder_producer, decoder_consumer) = HeapRb::<f32>::new(8192 * 2).split();
        let (resampler_output_producer, resampler_output_consumer) =
            HeapRb::<f32>::new(8192 * 2).split();

        let input_stream = create_input_stream(
            input_config.channels as usize,
            input_device,
            &input_config,
            input_producer,
        );
        let resampler_input_thread_run = create_resampler_thread(
            1,
            input_config.sample_rate.0 as usize,
            48000_usize,
            input_consumer,
            resampler_input_producer,
        );
        let denoise_thread_run =
            create_denoise_thread(1, resampler_input_consumer, denoise_producer);
        let encoder_thread_run =
            create_opus_encoder_thread(denoise_consumer, 48000_u32, 1, socket_sender);
        let decoder_thread_run = create_opus_decoder_thread(decoder_producer, 48000_u32, 1, socket);
        let resampler_output_thread_run = create_resampler_thread(
            1,
            48000_usize,
            output_config.sample_rate.0 as usize,
            decoder_consumer,
            resampler_output_producer,
        );
        let output_stream = create_output_stream(
            output_config.channels as usize,
            output_device,
            &output_config,
            resampler_output_consumer,
        );

        input_stream.play().expect("Failed to play input stream");
        output_stream.play().expect("Failed to play output stream");

        Self {
            input_stream,
            output_stream,
            resampler_input_thread_run,
            denoise_thread_run,
            encoder_thread_run,
            decoder_thread_run,
            resampler_output_thread_run,
        }
    }
}

impl Drop for P2P {
    fn drop(&mut self) {
        self.denoise_thread_run.store(false, Ordering::Relaxed);
        self.resampler_input_thread_run
            .store(false, Ordering::Relaxed);
        self.encoder_thread_run.store(false, Ordering::Relaxed);
        self.decoder_thread_run.store(false, Ordering::Relaxed);
        self.resampler_output_thread_run
            .store(false, Ordering::Relaxed);
    }
}
