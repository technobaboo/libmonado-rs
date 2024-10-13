use clap::Parser;
use libmonado::Monado;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
	monado_lib_path: Option<PathBuf>,
}

fn main() {
	let args = Cli::parse();
	let monado = if let Some(monado_lib_path) = args.monado_lib_path {
		Monado::create(monado_lib_path).unwrap()
	} else {
		Monado::auto_connect().unwrap()
	};
	dbg!(monado.get_api_version());
	println!();

	for mut client in monado.clients().unwrap() {
		dbg!(client.name().unwrap(), client.state().unwrap());
		println!();
	}
	for device in monado.devices().unwrap() {
		let _ = dbg!(device.name_id, device.serial());
		println!();
	}
	for tracking_origin in monado.tracking_origins().unwrap() {
		dbg!(
			tracking_origin.id,
			&tracking_origin.name,
			tracking_origin.get_offset().unwrap()
		);
		println!();
	}
}
