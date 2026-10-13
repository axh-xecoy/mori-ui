use anyhow::{Context, Result, bail};
use cpal::{
    FromSample, Sample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Runs only after the user presses the microphone button. Audio stays local until transcription.
pub fn record(stop: &AtomicBool, path: PathBuf) -> Result<PathBuf> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("未找到麦克风，请检查系统音频输入")?;
    let supported = device.default_input_config()?;
    let config: cpal::StreamConfig = supported.clone().into();
    let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
    let error = Arc::new(Mutex::new(None::<String>));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => input::<f32>(&device, &config, samples.clone(), error.clone())?,
        cpal::SampleFormat::I16 => input::<i16>(&device, &config, samples.clone(), error.clone())?,
        cpal::SampleFormat::U16 => input::<u16>(&device, &config, samples.clone(), error.clone())?,
        cpal::SampleFormat::I32 => input::<i32>(&device, &config, samples.clone(), error.clone())?,
        format => bail!("麦克风采样格式暂不支持：{format}"),
    };
    stream.play()?;
    let start = Instant::now();
    while !stop.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(120) {
        if let Some(e) = error.lock().unwrap().take() {
            bail!("麦克风录音失败：{e}");
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    drop(stream);
    let samples = samples.lock().unwrap();
    if samples.len() < config.sample_rate.0 as usize / 5 {
        bail!("录音太短，请至少录制一秒");
    }
    let spec = hound::WavSpec {
        channels: config.channels,
        sample_rate: config.sample_rate.0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec)?;
    for &sample in samples.iter() {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(path)
}
fn input<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<i16>>>,
    error: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample + Sample,
    i16: FromSample<T>,
{
    let limit = config.sample_rate.0 as usize * config.channels as usize * 120;
    Ok(device.build_input_stream(
        config,
        move |data: &[T], _| {
            if let Ok(mut samples) = samples.try_lock() {
                let remaining = limit.saturating_sub(samples.len());
                samples.extend(data.iter().take(remaining).map(|&s| i16::from_sample(s)));
            }
        },
        move |err| {
            *error.lock().unwrap() = Some(err.to_string());
        },
        None,
    )?)
}
