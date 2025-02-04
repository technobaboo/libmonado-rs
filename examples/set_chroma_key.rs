use clap::Parser;
use color::{OpaqueColor, Srgb};
use libmonado::Monado;
use std::{path::PathBuf, str::FromStr};

#[derive(Parser)]
struct Cli {
	color: String,
	hue_span: f32,
	saturation_span: f32,
	value_span: f32,
	curve_power: f32,
	despill: f32,
	monado_lib_path: Option<PathBuf>,
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
	let max = r.max(g).max(b);
	let min = r.min(g).min(b);
	let delta = max - min;

	let h = if delta == 0.0 {
		0.0
	} else if max == r {
		(((g - b) / delta) / 6.0 + 1.0) % 1.0
	} else if max == g {
		((b - r) / delta + 2.0) / 6.0
	} else {
		((r - g) / delta + 4.0) / 6.0
	};

	let s = if max == 0.0 { 0.0 } else { delta / max };
	let v = max;

	(h, s, v)
}

fn main() {
	let args = Cli::parse();
	let monado = if let Some(monado_lib_path) = args.monado_lib_path {
		Monado::create(monado_lib_path).unwrap()
	} else {
		Monado::auto_connect().unwrap()
	};

	let center_color: OpaqueColor<Srgb> = OpaqueColor::from_str(&args.color).unwrap();
	let [r, g, b] = center_color.components;
	let (h, s, v) = rgb_to_hsv(r, g, b);

	let h_min = (h - args.hue_span * 0.5).rem_euclid(1.0);
	let h_max = (h + args.hue_span * 0.5).rem_euclid(1.0);
	let s_min = (s - args.saturation_span * 0.5).clamp(0.0, 1.0);
	let s_max = (s + args.saturation_span * 0.5).clamp(0.0, 1.0);
	let v_min = (v - args.value_span * 0.5).clamp(0.0, 1.0);
	let v_max = (v + args.value_span * 0.5).clamp(0.0, 1.0);

	monado
		.set_chroma_key_params(
			h_min..h_max,
			s_min..s_max,
			v_min..v_max,
			args.curve_power,
			args.despill,
		)
		.unwrap();
}
