use fdk_aac_rust::encoder::{ConfiguredPureRustEncoder, EncoderParameter, PureRustEncoderParameters};

fn encode_facade(bandwidth: Option<u32>, name: &str) {
    let dir = std::env::temp_dir().join("fdkprobe-out");
    let mut params = PureRustEncoderParameters::new(2);
    let mut settings = vec![
        (EncoderParameter::AudioObjectType, 2),
        (EncoderParameter::SampleRate, 48_000),
        (EncoderParameter::ChannelMode, 2),
        (EncoderParameter::BitrateMode, 0),
        (EncoderParameter::Bitrate, 192_000),
        (EncoderParameter::TransportMux, 2),
        (EncoderParameter::Afterburner, 1),
    ];
    if let Some(bw) = bandwidth {
        settings.push((EncoderParameter::Bandwidth, bw));
    }
    for (p, v) in settings {
        params.set_parameter(p, v).unwrap();
    }
    let mut enc = ConfiguredPureRustEncoder::from_parameters(&params).unwrap();
    println!(
        "{name} resolved: bw={} delay={} frame={}",
        enc.config().bandwidth,
        enc.encoder_delay(),
        enc.config().frame_length
    );
    let per_frame = enc.input_samples_per_channel() * 2;
    let mut out = Vec::new();
    let mut n_frames = 0usize;
    let mut i = 0usize;
    while i < 200 * 1024 * 2 {
        let end = (i + per_frame).min(200 * 1024 * 2);
        let mut buf = pcm_slice(i, end);
        buf.resize(per_frame, 0.0);
        let bytes = enc.encode_transport_f32(&buf).unwrap();
        if !bytes.is_empty() {
            n_frames += 1;
            out.extend_from_slice(&bytes);
        }
        i = end;
    }
    let silence = vec![0.0f32; per_frame];
    for _ in 0..3 {
        let bytes = enc.encode_transport_f32(&silence).unwrap();
        if !bytes.is_empty() {
            n_frames += 1;
            out.extend_from_slice(&bytes);
        }
    }
    std::fs::write(dir.join(name).with_extension("aac"), &out).unwrap();
    println!("{name}: {} ADTS frames, {} bytes", n_frames, out.len());
}

fn pcm_slice(from: usize, to: usize) -> Vec<f32> {
    (from..to)
        .map(|i| {
            let t = (i / 2) as f32 / 48_000.0;
            0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
        })
        .collect()
}

fn decode_peak(stem: &str) {
    let dir = std::env::temp_dir().join("fdkprobe-out");
    let path = dir.join(format!("{stem}.aac"));
    let out = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args(["-f", "f32le", "-acodec", "pcm_f32le", "-"])
        .output()
        .unwrap();
    if !out.status.success() {
        println!("{stem}: ffmpeg FAILED {}", String::from_utf8_lossy(&out.stderr));
        return;
    }
    let data = out.stdout;
    let mut peak: f32 = 0.0;
    let mut sum_sq = 0.0f64;
    let n = data.len() / 4;
    for c in data.chunks_exact(4) {
        let v = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        peak = peak.max(v.abs());
        sum_sq += (v as f64) * (v as f64);
    }
    let rms = if n > 0 { (sum_sq / n as f64).sqrt() } else { 0.0 };
    println!("{stem}: samples={n} peak={peak:.6} rms={rms:.6}");
}

fn main() {
    // A/B: facade variants
    encode_facade(Some(15_000), "a_facade");
    encode_facade(None, "b_facade_nobw");

    // C: low-level stereo
    {
        use fdk_aac_rust::aac_encoder::{write_adts_frame, PureRustAacLcStereoEncoder};
        let dir = std::env::temp_dir().join("fdkprobe-out");
        let nominal_bits = 192_000usize * 1024 / 48_000;
        let capacity = 6144 * 2 - nominal_bits;
        let mut enc =
            PureRustAacLcStereoEncoder::new_with_frame_length(3, 1024, nominal_bits, capacity)
                .unwrap();
        enc.set_bandwidth(24_000);
        enc.set_afterburner(true);
        let mut out = Vec::new();
        let mut n_frames = 0;
        let mut i = 0usize;
        let total = 200 * 1024 * 2;
        while i < total {
            let end = (i + 2048).min(total);
            let frame = pcm_slice(i, end);
            let mut l = Vec::with_capacity(1024);
            let mut r = Vec::with_capacity(1024);
            for pair in frame.chunks(2) {
                l.push(*pair.first().unwrap_or(&0.0));
                r.push(*pair.get(1).unwrap_or(&0.0));
            }
            while l.len() < 1024 {
                l.push(0.0);
                r.push(0.0);
            }
            let raw = enc.encode_raw_data_block(&l, &r).unwrap();
            if !raw.is_empty() {
                n_frames += 1;
                out.extend_from_slice(&write_adts_frame(&raw, 3, 2).unwrap());
            }
            i = end;
        }
        std::fs::write(dir.join("c_lowlevel.aac"), &out).unwrap();
        println!("C: {} ADTS frames, {} bytes", n_frames, out.len());
    }

    // D: low-level mono with bandwidth cap + enlarged reservoir
    {
        use fdk_aac_rust::aac_encoder::{write_adts_frame, PureRustAacLcMonoEncoder};
        let dir = std::env::temp_dir().join("fdkprobe-out");
        let nominal_bits = 96_000usize * 1024 / 48_000;
        let capacity = 6144usize * 4 - nominal_bits;
        let mut enc =
            PureRustAacLcMonoEncoder::new_with_frame_length(3, 1024, nominal_bits, capacity)
                .unwrap();
        enc.set_bandwidth(15_000);
        let mut out = Vec::new();
        let mut n_frames = 0;
        let mut i = 0usize;
        let total = 200 * 1024 * 2;
        while i < total {
            let end = (i + 2048).min(total);
            let frame = pcm_slice(i, end);
            let mono: Vec<f32> = frame.chunks(2).map(|p| p[0]).collect();
            let raw = enc.encode_raw_data_block(&mono).unwrap();
            if !raw.is_empty() {
                n_frames += 1;
                out.extend_from_slice(&write_adts_frame(&raw, 3, 1).unwrap());
            }
            i = end;
        }
        std::fs::write(dir.join("d_mono.aac"), &out).unwrap();
        println!("D: {} ADTS frames, {} bytes", n_frames, out.len());
    }

    // E: stage-by-stage diagnostics (stereo pipeline primitives)
    {
        use fdk_aac_rust::aac_encoder::{
            AacLcAnalysisFilterbank, AacLcPsychoacousticModel, AacLcQuantizer,
        };
        let mut fb = AacLcAnalysisFilterbank::new(1024).unwrap();
        let psy = AacLcPsychoacousticModel::new_with_frame_length(3, 1024).unwrap();
        let quant = AacLcQuantizer::new_with_frame_length(3, 1024).unwrap();
        let mut frame_in = vec![0.0f32; 2048];
        for (i, slot) in frame_in.chunks_mut(2).enumerate() {
            let t = i as f32 / 48_000.0;
            let v = 0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            slot[0] = v;
            slot[1] = v;
        }
        let _ = fb.analyze(&frame_in[..1024]);
        let analysis = fb.analyze(&frame_in[1024..]).unwrap();
        let spec_max = analysis.spectrum.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        println!("E: spectrum max={spec_max:.6}");
        let psycho = psy.analyze(&analysis.spectrum).unwrap();
        let thr_min = psycho
            .bands
            .iter()
            .map(|b| b.masking_threshold)
            .fold(f32::INFINITY, f32::min);
        println!(
            "E: bands={} masking_threshold min={thr_min:.3e}",
            psycho.bands.len()
        );
        let q = quant.quantize_long(&analysis.spectrum, &psycho, usize::MAX).unwrap();
        let coef_max = q
            .bands
            .iter()
            .flat_map(|b| b.coefficients.iter())
            .fold(0i32, |a, v| a.max(v.unsigned_abs() as i32));
        println!(
            "E: coefficient max={coef_max} estimated_spectral_bits={}",
            q.estimated_spectral_bits
        );
    }

    // F/G: self-decode both files with the crate's own transport decoder
    for stem in ["d_mono", "c_lowlevel"] {
        use fdk_aac_rust::transport::PureRustTransportDecoder;
        let dir = std::env::temp_dir().join("fdkprobe-out");
        let bytes = std::fs::read(dir.join(format!("{stem}.aac"))).unwrap();
        let mut dec = PureRustTransportDecoder::from_adts_frame(&bytes[..1024]).unwrap();
        dec.push_adts_bytes(&bytes[1024..]).unwrap();
        let frames = dec.drain_adts_interleaved_f32().unwrap();
        let count = frames.len();
        let peak = frames.iter().flatten().fold(0.0f32, |a, v| a.max(v.abs()));
        let n: usize = frames.iter().map(|f| f.len()).sum();
        let sum_sq: f64 = frames.iter().flatten().map(|v| (*v as f64) * (*v as f64)).sum();
        let rms = if n > 0 { (sum_sq / n as f64).sqrt() } else { 0.0 };
        let head: Vec<f32> = frames.iter().flatten().take(6).copied().collect();
        println!("{stem} self-decode: frames={count} peak={peak:.6} rms={rms:.6} head={head:?}");
    }

    // ffmpeg reports
    for stem in ["a_facade", "b_facade_nobw", "c_lowlevel", "d_mono"] {
        decode_peak(stem);
    }
}
