use clap::{arg, command, Command};
use humble_cli::humanize_bytes;
use humble_cli::run_future;
use humble_cli::ApiError;
use std::fs;
use std::path;
use tabled::{object::Columns, Alignment, Modify, Style};

fn main() {
    match run() {
        Err(e) => eprintln!("Error: {}", e),
        _ => {}
    }
}

fn run() -> Result<(), anyhow::Error> {
    let session_key = std::fs::read_to_string("session.key")
        .expect("failed to read the session key from `session.key` file");

    let session_key = session_key.trim_end();

    let list_subcommand = Command::new("list").about("List all purchases");

    let details_subcommand = Command::new("details")
        .about("Print details of a certain product")
        .arg(arg!([KEY]).required(true));

    let download_subcommand = Command::new("download")
        .about("Download all items in a product")
        .arg(arg!([KEY]).required(true));

    let sub_commands = vec![list_subcommand, details_subcommand, download_subcommand];

    let matches = command!()
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommands(sub_commands)
        .get_matches();

    match matches.subcommand() {
        Some(("list", _)) => list_products(&session_key),
        Some(("details", sub_matches)) => {
            show_product_details(&session_key, sub_matches.value_of("KEY").unwrap())
        }
        Some(("download", sub_matches)) => {
            download_product(&session_key, sub_matches.value_of("KEY").unwrap())?
        }
        _ => {}
    }

    Ok(())
}

fn list_products(session_key: &str) {
    let api = humble_cli::HumbleApi::new(session_key);
    let products = api.list_products().unwrap();
    println!("Done: {} products", products.len());
    let mut builder = tabled::builder::Builder::default().set_columns(["Key", "Name", "Size"]);
    for p in products {
        builder = builder.add_record([
            &p.gamekey,
            &p.details.human_name,
            &humanize_bytes(p.total_size()),
        ]);
    }

    let table = builder
        .build()
        .with(Style::psql())
        .with(Modify::new(Columns::single(1)).with(Alignment::left()))
        .with(Modify::new(Columns::single(2)).with(Alignment::right()));
    println!("{table}");
}

fn show_product_details(session_key: &str, product_key: &str) {
    let api = humble_cli::HumbleApi::new(session_key);
    let product = api.read_product(product_key).unwrap();

    println!("");
    println!("{}", product.details.human_name);
    println!("Total size: {}", humanize_bytes(product.total_size()));
    println!("");

    // Items in this product
    let mut builder =
        tabled::builder::Builder::default().set_columns(["", "Name", "Format", "Total Size"]);

    for (idx, entry) in product.entries.iter().enumerate() {
        builder = builder.add_record([
            &idx.to_string(),
            &entry.human_name,
            &entry.formats(),
            &humanize_bytes(entry.total_size()),
        ]);
    }
    let table = builder
        .build()
        .with(Style::psql())
        .with(Modify::new(Columns::single(0)).with(Alignment::right()))
        .with(Modify::new(Columns::single(1)).with(Alignment::left()))
        .with(Modify::new(Columns::single(2)).with(Alignment::left()))
        .with(Modify::new(Columns::single(3)).with(Alignment::right()));
    println!("{table}");
}

fn download_product(session_key: &str, product_key: &str) -> Result<(), anyhow::Error> {
    let api = humble_cli::HumbleApi::new(session_key);
    let product = match api.read_product(product_key) {
        Ok(p) => p,
        Err(e) => match e {
            ApiError::BadHttpStatus(404) => {
                eprintln!("Error: Product not found");
                return Ok(());
            }
            _ => return Err(e.into()),
        },
    };

    let dir_name = humble_cli::replace_invalid_chars_in_filename(&product.details.human_name);
    let product_dir = create_dir(&dir_name)?;

    let client = reqwest::Client::new();

    for product_entry in product.entries {
        println!("");
        println!("{}", product_entry.human_name);

        let entry_dir = product_dir.join(product_entry.human_name);
        if !entry_dir.exists() {
            fs::create_dir(&entry_dir)?;
        }

        for download_entry in product_entry.downloads {
            for ele in download_entry.sub_items {
                // TODO: Don't use unwrap here
                let filename = humble_cli::extract_filename_from_url(&ele.url.web).unwrap();
                let download_path = entry_dir.join(&filename);
                //println!(" URL  =   {}", ele.url.web);
                //println!(" FILE =   {}", filename.to_str().unwrap());
                //println!("");

                let f = humble_cli::download_file(
                    &client,
                    &ele.url.web,
                    download_path.to_str().unwrap(),
                    &filename,
                );
                run_future(f).expect("failed to download the file");
            }
        }
    }

    Ok(())
}

fn create_dir(dir: &str) -> Result<path::PathBuf, std::io::Error> {
    let dir = path::Path::new(dir).to_owned();
    if !dir.exists() {
        fs::create_dir(&dir)?;
    }
    Ok(dir)
}
