use anyhow::{anyhow, Result};
use clap::Parser;
use log::info;
use skyprompt_audio::create_whisper_channel;
use skyprompt_audio::default_input_device;
use skyprompt_audio::default_output_device;
use skyprompt_audio::list_audio_devices;
use skyprompt_audio::parse_audio_device;
use skyprompt_audio::record_and_transcribe;
use skyprompt_audio::vad_engine::VadSensitivity;
use skyprompt_audio::AudioDevice;
use skyprompt_audio::AudioStream;
use skyprompt_audio::AudioTranscriptionEngine;
use skyprompt_audio::VadEngineEnum;
use skyprompt_core::Language;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(
        short,
        long,
        help = "Audio device name (can be specified multiple times)"
    )]
    audio_device: Vec<String>,

    #[clap(long, help = "List available audio devices")]
    list_audio_devices: bool,

    #[clap(short = 'l', long, value_enum)]
    language: Vec<Language>,

    #[clap(long, help = "Deepgram API key")]
    deepgram_api_key: Option<String>,
}

fn print_devices(devices: &[AudioDevice]) {
    println!("Available audio devices:");
    for device in devices.iter() {
        println!("  {}", device);
    }

    #[cfg(target_os = "macos")]
    println!("On macOS, it's not intuitive but output devices are your displays");
}

// ! usage - cargo run --bin skyprompt-audio -- --audio-device "Display 1 (output)"

#[tokio::main]
async fn main() -> Result<()> {
    use env_logger::Builder;
    use log::LevelFilter;

    Builder::new()
        .filter(None, LevelFilter::Debug)
        .filter_module("tokenizers", LevelFilter::Error)
        .init();

    let args = Args::parse();

    let devices = list_audio_devices().await?;

    if args.list_audio_devices {
        print_devices(&devices);
        return Ok(());
    }

    let languages = args.language;

    let deepgram_api_key = args.deepgram_api_key;

    let devices = if args.audio_device.is_empty() {
        vec![default_input_device()?, default_output_device()?]
    } else {
        args.audio_device
            .iter()
            .map(|d| parse_audio_device(d))
            .collect::<Result<Vec<_>>>()?
    };

    if devices.is_empty() {
        return Err(anyhow!("No audio input devices found"));
    }

    // delete .mp4 files (output*.mp4)
    std::fs::remove_file("output_0.mp4").unwrap_or_default();
    std::fs::remove_file("output_1.mp4").unwrap_or_default();

    let chunk_duration = Duration::from_secs(10);
    let output_path = PathBuf::from("output.mp4");
    let (whisper_sender, whisper_receiver, _) = create_whisper_channel(
        Arc::new(AudioTranscriptionEngine::WhisperDistilLargeV3),
        VadEngineEnum::WebRtc, // Or VadEngineEnum::WebRtc, hardcoded for now
        deepgram_api_key,
        &output_path,
        VadSensitivity::Medium,
        languages,
        None
    )
    .await?;
    // Spawn threads for each device
    let recording_threads: Vec<_> = devices
        .into_iter()
        .map(|device| {
            let device = Arc::new(device);
            let whisper_sender = whisper_sender.clone();
            let device_control = Arc::new(AtomicBool::new(true));
            let device_clone = Arc::clone(&device);

            tokio::spawn(async move {
                let device_control_clone = Arc::clone(&device_control);
                let device_clone_2 = Arc::clone(&device_clone);
                let audio_stream =
                    AudioStream::from_device(device_clone_2, device_control_clone.clone())
                        .await
                        .unwrap();

                record_and_transcribe(
                    Arc::new(audio_stream),
                    chunk_duration,
                    whisper_sender,
                    device_control_clone,
                )
                .await
            })
        })
        .collect();
    let mut consecutive_timeouts = 0;
    let max_consecutive_timeouts = 3; // Adjust this value as needed

    // Main loop to receive and print transcriptions
    loop {
        match whisper_receiver.try_recv() {
            Ok(result) => {
                info!("Transcription: {:?}", result);
                consecutive_timeouts = 0; // Reset the counter on successful receive
            }
            Err(_) => {
                consecutive_timeouts += 1;
                if consecutive_timeouts >= max_consecutive_timeouts {
                    info!("No transcriptions received for a while, stopping...");
                    break;
                }
                continue;
            }
        }
    }

    // Wait for all recording threads to finish
    for (i, thread) in recording_threads.into_iter().enumerate() {
        let file_path = thread.await.unwrap();
        println!("Recording {} complete: {:?}", i, file_path);
    }

    Ok(())
}
