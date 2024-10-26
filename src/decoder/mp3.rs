use std::io::{Read, Seek, SeekFrom};
use std::time::Duration;

use crate::source::SeekError;
use crate::Source;

use minimp3::SeekDecoder;
use minimp3::Frame;
use minimp3_fixed as minimp3;

pub struct Mp3Decoder<R>
where
    R: Read + Seek,
{
    decoder: SeekDecoder<R>,
    current_frame: Frame,
    current_frame_offset: usize,
}

impl<R> Mp3Decoder<R>
where
    R: Read + Seek,
{
    pub fn new(mut data: R) -> Result<Self, R> {
        if !is_mp3(data.by_ref()) {
            return Err(data);
        }
        let mut decoder = SeekDecoder::new(data)
        // parameters are correct and minimp3 is used correctly
        // thus if we crash here one of these invariants is broken:
            .expect("should be able to allocate memory, perform IO");
        let current_frame = decoder.decode_frame()
            // the reader makes enough data available therefore
            // if we crash here the invariant broken is:
            .expect("data should not corrupt");

        Ok(Mp3Decoder {
            decoder,
            current_frame,
            current_frame_offset: 0,
        })
    }
    pub fn into_inner(self) -> R {
        self.decoder.into_inner()
    }
}

impl<R> Source for Mp3Decoder<R>
where
    R: Read + Seek,
{
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.current_frame.data.len())
    }

    #[inline]
    fn channels(&self) -> u16 {
        self.current_frame.channels as _
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        self.current_frame.sample_rate as _
    }

    #[inline]
    fn total_duration(&self) -> Option<Duration> {
        None
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        // TODO waiting for PR in minimp3_fixed or minimp3
        // nah xd :3

        let pos = (pos.as_secs_f32() * self.sample_rate() as f32) as u64;
        // do not trigger a sample_rate, channels and frame len update
        // as the seek only takes effect after the current frame is done
        self.decoder.seek_samples(pos)
            // there is no guarntee that this code is going to work, like at all, therefore
            // if we crash here, the invariant broken is:
            .expect("minimp3 c library shouldn't throw error because you probably caused ub:wqa");
        Ok(())
    }
}

impl<R> Iterator for Mp3Decoder<R>
where
    R: Read + Seek,
{
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.current_frame_offset == self.current_frame_len().unwrap() {
            if let Ok(frame) = self.decoder.decode_frame() {
                self.current_frame = frame;
                self.current_frame_offset = 0;
            } else {
                return None;
            }
        }

        let v = self.current_frame.data[self.current_frame_offset];
        self.current_frame_offset += 1;

        Some(v)
    }
}

/// Returns true if the stream contains mp3 data, then resets it to where it was.
fn is_mp3<R>(mut data: R) -> bool
where
    R: Read + Seek,
{
    let stream_pos = data.seek(SeekFrom::Current(0)).unwrap();
    // TODO(echotune): given that echotune uses its own in-house solution for determing file types
    // (which isn't as reliable as this one, because echotune simply checks the file signature, as
    // opposed to trying to decode a frame)
    // but yeah yk wtf who cares actually this uses methods needed for playing in the first place
    // it only increases binary size by like 100 bytes nobody fucking cares ettc
    let mut decoder = SeekDecoder::new(data.by_ref()).unwrap();
    let ok = decoder.decode_frame().is_ok();
    data.seek(SeekFrom::Start(stream_pos)).unwrap();

    ok
}
