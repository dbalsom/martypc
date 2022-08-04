use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{Producer, Consumer, RingBuffer};
use std::fs::File;
use std::io::Write;

pub const VOLUME_ADJUST: f32 = 0.10;
pub const BUFFER_MS: f32 = 30.0;

pub struct SoundPlayer {

    audio_device: cpal::Device,
    //audio_config_s: cpal::SupportedStreamConfig,
    //audio_config: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    sample_rate: u32,
    channels: usize,

    pub samples_consumed: u64,
    pub samples_produced: u64,

    pub buffer_producer: Producer<f32>,
    output_stream: cpal::Stream,
}

impl SoundPlayer {
    pub fn get_sample_format() -> cpal::SampleFormat {
        let audio_device = cpal::default_host()
            .default_output_device()
            .expect("Failed to get default output audio device.");        

        audio_device.default_output_config()
            .expect("Failed to get default sample format.")
            .sample_format()
    }

    pub fn new<T>() -> Self
    where
        T: cpal::Sample,
    {
        let host = cpal::default_host();
        let audio_device = host
            .default_output_device()
            .expect("Failed to get default output audio device.");
            
        let config = audio_device.default_output_config().unwrap();    
        
        let sample_format = config.sample_format();
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        
        let min_buffer = ((BUFFER_MS / 1000.0) / (1.0 / sample_rate as f32)) as usize;
        log::trace!("Minimum sample buffer size: {}", min_buffer);
        //let buffer_size = (sample_rate as f32 * (BUFFER_MS as f32 / 1000.0)) as usize;

        let buffer_size = sample_rate;
        let buffer = RingBuffer::new(buffer_size as usize );
        let (buffer_producer, mut buffer_consumer) = buffer.split();

        let err_fn = |err| eprintln!("An error occurred during streaming: {}", err);

        //let mut debug_snd_file = File::create("output2.pcm").expect("Couldn't open debug pcm file");

        let mut consumer_count: u64 = 0;
        let mut last_value: f32 = 0.0;
        let mut refill_buffer: bool = true;
        let mut next_value = move || {
            consumer_count += 1;
            //log::trace!("consumer: {}", consumer_count);

            if refill_buffer {
                if buffer_consumer.len() < min_buffer {
                    return 0.0
                }
                else {
                    refill_buffer = false;
                }
            }

            let sample: f32 = match buffer_consumer.pop() {
                Some(s) => {
                    s
                }
                None => {
                    //log::trace!("Buffer underrun");
                    refill_buffer = true;
                    0.0
                }
            };
            //debug_snd_file.write(&s.to_be_bytes());
            sample
        };

        let output_stream = audio_device
            .build_output_stream(
                &config.into(),
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    write_data(data, channels, &mut next_value)
                },
                err_fn)
            .expect("Failed to build an output audio stream");


        Self {
            audio_device,
            //audio_config_s: config,
            //audio_config: config.into(),
            sample_format,
            sample_rate,
            samples_consumed: 0,
            samples_produced: 0,
            channels,
            buffer_producer,
            output_stream,
        }
    }

    pub fn play(&self) {
        self.output_stream.play().unwrap();
    }

    pub fn queue_sample(&mut self, data: f32) {
        match self.buffer_producer.push(data) {
            Ok(_) => {},
            Err(_) => {}
        }
    }
    
    pub fn queue_sample_slice(&mut self, data: &[f32]) {
        self.buffer_producer.push_slice(data);
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: cpal::Sample,
{
    for frame in output.chunks_mut(channels) {
        
        let value: T = cpal::Sample::from::<f32>(&next_sample());

        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}