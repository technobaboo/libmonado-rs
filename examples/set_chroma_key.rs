use clap::Parser;
use color::OpaqueColor;
use libmonado::Monado;
use std::{path::PathBuf, str::FromStr};

#[derive(Parser)]
struct Cli {
	color: String,
	threshold: f32,
	smoothing: f32,
	monado_lib_path: Option<PathBuf>,
}

fn main() {
	let args = Cli::parse();
	let monado = if let Some(monado_lib_path) = args.monado_lib_path {
		Monado::create(monado_lib_path).unwrap()
	} else {
		Monado::auto_connect().unwrap()
	};

	monado
		.set_chroma_key_params(
			OpaqueColor::from_str(&args.color).unwrap(),
			args.threshold,
			args.smoothing,
		)
		.unwrap();
}
