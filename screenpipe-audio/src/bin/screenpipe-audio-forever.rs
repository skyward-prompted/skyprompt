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

    #[clap(long, help = "Audio chunk duration in seconds")]
    audio_chunk_duration: f32,

    #[clap(long, help = "Deepgram API key")]
    deepgram_api_key: Option<String>,

    #[clap(short = 'l', long, value_enum)]
    language: Vec<Language>,
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

    let languages = args.language;

    let devices = list_audio_devices().await?;

    if args.list_audio_devices {
        print_devices(&devices);
        return Ok(());
    }

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

    let chunk_duration = Duration::from_secs_f32(args.audio_chunk_duration);
    let (whisper_sender, whisper_receiver, _) = create_whisper_channel(
        Arc::new(AudioTranscriptionEngine::WhisperDistilLargeV3),
        VadEngineEnum::Silero, // Or VadEngineEnum::WebRtc, hardcoded for now
        args.deepgram_api_key,
        &PathBuf::from("output.mp4"),
        VadSensitivity::Medium,
        languages,
        None,
    )
    .await?;

    let whisper_sender_clone = whisper_sender.clone();
    // Spawn threads for each device
    let _recording_threads: Vec<_> = devices
        .into_iter()
        .enumerate()
        .map(|(i, device)| {
            let whisper_sender = whisper_sender_clone.clone();
            async move {
                let device = Arc::new(device);
                let device_control = Arc::new(AtomicBool::new(true));

                let audio_stream = AudioStream::from_device(device.clone(), device_control.clone())
                    .await
                    .unwrap();

                tokio::spawn(async move {
                    loop {
                        let result = record_and_transcribe(
                            Arc::new(audio_stream.clone()),
                            chunk_duration,
                            whisper_sender.clone(),
                            Arc::clone(&device_control),
                        )
                        .await;

                        if let Err(e) = result {
                            eprintln!("Error in recording thread {}: {:?}", i, e);
                            // Optionally add a short delay before retrying
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                    }
                })
                .await
            }
        })
        .collect();

    // Main loop to receive and print transcriptions
    loop {
        match whisper_receiver.recv() {
            Ok(result) => {
                info!("Transcription: {:?}", result);
            }
            Err(e) => {
                eprintln!("Error receiving transcription: {:?}", e);
            }
        }
    }
}
