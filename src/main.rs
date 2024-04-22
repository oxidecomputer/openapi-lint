use std::{env, fs::File, path::Path, process};

use openapiv3::OpenAPI;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() != 2 {
        println!("usage: openapi-linter filename");
    }
    let filename = &args[1];

    //let content = fs::read_to_string(filename).expect("Unable to read file");

    // Make sure the result parses as a valid OpenAPI spec.
    let spec = load_api(&filename);

    // Check for lint errors.
    let errors = openapi_lint::validate(&spec);
    if !errors.is_empty() {
        eprintln!("{}", errors.join("\n\n"));
        process::exit(1);
    }
}

fn load_api<P>(path: P) -> OpenAPI
where
    P: AsRef<Path> + std::clone::Clone + std::fmt::Debug,
{
    let mut file = File::open(path.clone()).unwrap();
    match serde_json::from_reader(file) {
        Ok(json_value) => json_value,
        _ => {
            file = File::open(path.clone()).unwrap();
            serde_yaml::from_reader(file).expect("file was not valid OpenAPI")
        }
    }
}
