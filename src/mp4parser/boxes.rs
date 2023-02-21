/*
    REFERENCES
    ----------

    1. https://github.com/shaka-project/shaka-player/blob/d465942c4393e6c891d6a230bea90a44d90cc70b/lib/util/mp4_box_parsers.js

*/

use super::Reader;
use std::io::Result;

// #[allow(dead_code, clippy::upper_case_acronyms)]

pub(super) struct ParsedTFHDBox {
    /// As per the spec: an integer that uniquely identifies this
    /// track over the entire life‐time of this presentation
    track_id: u32,
    /// If specified via flags, this overrides the default sample
    /// duration in the Track Extends Box for this fragment
    default_sample_duration: Option<u32>,
    /// If specified via flags, this overrides the default sample
    /// size in the Track Extends Box for this fragment
    default_sample_size: Option<u32>,
    /// If specified via flags, this indicate the base data offset
    base_data_offset: Option<u64>,
}

impl ParsedTFHDBox {
    /// Parses a TFHD Box.
    pub(super) fn parse(reader: &mut Reader, flags: u32) -> Result<Self> {
        let mut default_sample_duration = None;
        let mut default_sample_size = None;
        let mut base_data_offset = None;

        let track_id = reader.read_u32()?; // Read "track_ID"

        // Skip "base_data_offset" if present.
        if (flags & 0x000001) != 0 {
            base_data_offset = Some(reader.read_u64()?);
        }

        // Skip "sample_description_index" if present.
        if (flags & 0x000002) != 0 {
            reader.skip(4)?;
        }

        // Read "default_sample_duration" if present.
        if (flags & 0x000008) != 0 {
            default_sample_duration = Some(reader.read_u32()?);
        }

        // Read "default_sample_size" if present.
        if (flags & 0x000010) != 0 {
            default_sample_size = Some(reader.read_u32()?);
        }

        Ok(Self {
            track_id,
            default_sample_duration,
            default_sample_size,
            base_data_offset,
        })
    }
}

pub(super) struct ParsedTFDTBox {
    /// As per the spec: the absolute decode time, measured on the media
    /// timeline, of the first sample in decode order in the track fragment
    base_media_decode_time: u64,
}

impl ParsedTFDTBox {
    /// Parses a TFDT Box.
    pub(super) fn parse(reader: &mut Reader, version: u32) -> Result<Self> {
        Ok(Self {
            base_media_decode_time: if version == 1 {
                reader.read_u64()?
            } else {
                reader.read_u32()? as u64
            },
        })
    }
}

pub(super) struct ParsedMDHDBox {
    /// As per the spec: an integer that specifies the time‐scale for this media;
    /// this is the number of time units that pass in one second
    timescale: u32,
    /// Language code for this media
    language: String,
}

impl ParsedMDHDBox {
    /// Parses a MDHD Box.
    pub(super) fn parse(reader: &mut Reader, version: u32) -> Result<Self> {
        if version == 1 {
            reader.skip(8)?; // Skip "creation_time"
            reader.skip(8)?; // Skip "modification_time"
        } else {
            reader.skip(4)?; // Skip "creation_time"
            reader.skip(4)?; // Skip "modification_time"
        }

        let timescale = reader.read_u32()?;

        reader.skip(4)?; // Skip "duration"

        let language = reader.read_u16()?;

        // language is stored as an ISO-639-2/T code in an array of three
        // 5-bit fields each field is the packed difference between its ASCII
        // value and 0x60
        let language_string = String::from_utf16(&[
            (language >> 10) + 0x60,
            ((language & 0x03c0) >> 5) + 0x60,
            (language & 0x1f) + 0x60,
        ])
        .unwrap_or("".to_owned());

        Ok(Self {
            timescale,
            language: language_string,
        })
    }
}

pub(super) struct ParsedTRUNBox {
    /// As per the spec: the number of samples being added in this run;
    sample_count: u32,
    ///  An array of size <sampleCount> containing data for each sample
    sample_data: Vec<ParsedTRUNSample>,
    /// If specified via flags, this indicate the offset of the sample in bytes.
    data_offset: Option<u32>,
}

impl ParsedTRUNBox {
    /// Parses a TRUN Box.
    pub(super) fn parse(reader: &mut Reader, flags: u32, version: u32) -> Result<Self> {
        let sample_count = reader.read_u32()?;
        let mut sample_data = vec![];
        let mut data_offset = None;

        // "data_offset"
        if (flags & 0x000001) != 0 {
            data_offset = Some(reader.read_u32()?);
        }

        // Skip "first_sample_flags" if present.
        if (flags & 0x000004) != 0 {
            reader.skip(4)?;
        }

        for i in 0..sample_count {
            let mut sample = ParsedTRUNSample {
                sample_duration: None,
                sample_size: None,
                sample_composition_time_offset: None,
            };

            // Read "sample duration" if present.
            if (flags & 0x000100) != 0 {
                sample.sample_duration = Some(reader.read_u32()?);
            }

            // Read "sample_size" if present.
            if (flags & 0x000200) != 0 {
                sample.sample_size = Some(reader.read_u32()?);
            }

            // Skip "sample_flags" if present.
            if (flags & 0x000400) != 0 {
                reader.skip(4)?;
            }

            // Read "sample_time_offset" if present.
            if (flags & 0x000800) != 0 {
                sample.sample_composition_time_offset = Some(if version == 0 {
                    reader.read_u32()? as i32
                } else {
                    reader.read_i32()?
                });
            }

            sample_data.push(sample);
        }

        Ok(Self {
            sample_count,
            sample_data,
            data_offset,
        })
    }
}

pub(super) struct ParsedTRUNSample {
    /// The length of the sample in timescale units.
    sample_duration: Option<u32>,
    /// The size of the sample in bytes.
    sample_size: Option<u32>,

    /// The time since the start of the sample in timescale units. Time
    /// offset is based of the start of the sample. If this value is
    /// missing, the accumulated durations preceeding this time sample will
    /// be used to create the start time.
    sample_composition_time_offset: Option<i32>,
}
