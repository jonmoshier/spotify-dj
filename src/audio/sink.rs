use librespot_playback::{
    audio_backend::{Sink, SinkResult},
    convert::Converter,
    decoder::AudioPacket,
};
use std::sync::mpsc::SyncSender;

/// Wraps a real audio sink and tees raw PCM samples to a channel for analysis.
pub struct TeeSink {
    inner: Box<dyn Sink>,
    pcm_tx: SyncSender<Vec<f64>>,
}

impl TeeSink {
    pub fn new(inner: Box<dyn Sink>, pcm_tx: SyncSender<Vec<f64>>) -> Self {
        Self { inner, pcm_tx }
    }
}

impl Sink for TeeSink {
    fn start(&mut self) -> SinkResult<()> {
        self.inner.start()
    }

    fn stop(&mut self) -> SinkResult<()> {
        self.inner.stop()
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        // Borrow samples before moving packet into inner sink.
        if let AudioPacket::Samples(ref samples) = packet {
            // try_send: drop the chunk if the detector is backed up — never block the audio thread.
            let _ = self.pcm_tx.try_send(samples.clone());
        }
        self.inner.write(packet, converter)
    }
}
