use clap::Parser;
use libmonado_rs::Monado;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
	monado_lib_path: PathBuf,
}

fn main() {
	let args = Cli::parse();
	let monado = Monado::create(args.monado_lib_path).unwrap();
	dbg!(monado.get_api_version());
	for mut client in monado.clients().unwrap() {
		println!(
			"Client name is {} and state is {:?}",
			client.name().unwrap(),
			client.state().unwrap()
		)
	}
	for device in monado.devices().unwrap() {
		dbg!(device);
	}
}
