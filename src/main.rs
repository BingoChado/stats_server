use warp::Filter;
use clap::{Arg, App, SubCommand};
use std::convert::Infallible;
use std::io::Read;

mod handler;
mod config;
mod log;
mod database;
mod helpers;
use log::{
    log,
    LTYPE
};

// reference for sharing db between async things
//https://github.com/andrewleverette/rust_warp_api/blob/master/src/handlers.rs

async fn run_server(config_struct: config::Config, dbpath: Option<String>) {
    log(LTYPE::Info, format!("Started web server"));

    // create a new mutex for a handler object
    let db = match database::init_db(dbpath) {
            Ok(a) => a,
            Err(e) => panic!("Failed to open database! ({})", e)
        };

    // favicon path filter
    //let favico = warp::path("favicon.ico").and(warp::fs::file("favicon.ico"));

    // api filters
    let api_root = warp::path!("api"/..);
    
    // define the POST api
    let push = warp::path!("post" / String)
        .and(warp::post())
        .and(json_body())
        .and(with_db(db.clone()))
        .and_then( move |uuid, body, db1: database::Db|  {
                    handler::post_handle(uuid, body, db1)
            }
        );

    // define the GET api
    let get = warp::path!("get" / String / String)
        .and(with_db(db.clone()))
        .and_then(move |uuid, encdat, db2| {
                    handler::get_handle(uuid, encdat, db2)        
            } 
            
        );
    
    // define the ADMIN api
    let adm = warp::path!("adm" / String / String)
        .and(warp::path::end())
        .and(with_db(db))
        .and_then( move |command, body, db3| {
                    handler::adm_handle(command, body, db3)
            }
        );

    let api = api_root
        .and(
            push
            .or(get)
            .or(adm)
        );
    //let routes = favico
    //    .or(hi);
    // run the server, and point it to the keys
    warp::serve(api)
        .tls()
        .cert_path("./cert.pem")
        .key_path("./key.pem")
        .run(([127,0,0,1],config_struct.port()))
        .await;
}

fn with_db(db: database::Db) -> impl Filter<Extract = (database::Db,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

fn json_body() -> impl Filter<Extract = (database::DatabaseVar,), Error = warp::Rejection> + Clone {
    // When accepting a body, we want a JSON body
    // (and to reject huge payloads)...
    warp::body::content_length_limit(1024 * 16).and(warp::body::json())
}

#[tokio::main]
async fn main() {
    // parse the CLI arguments
    let matches =  App::new("Stats Server")
                        .version("0.1")
                        .about("Serves up statistics")
                        .subcommand(SubCommand::with_name("new")
                            .about("Creates a new config file")
                            .version("1.0")
                            .arg(Arg::with_name("file")
                                .short("f")
                                .long("file")
                                .help("Saves config to a file")
                                .value_name("FILE")
                                .takes_value(true)
                            )
                            .arg(Arg::with_name("number")
                                .short("n")
                                .long("number")
                                .help("The number of access entries for the config file")
                                .value_name("NUM")
                                .takes_value(true)
                                .required(true)
                            )
                        )
                        .subcommand(SubCommand::with_name("run")
                            .about("Run the server")
                            .version("0.1")
                            .arg(Arg::with_name("config")
                                .short("c")
                                .long("config")
                                .value_name("FILE")
                                .help("Uses FILE as the config file of the server")
                                .takes_value(true)
                                .required(true)
                            ).arg(Arg::with_name("database")
                                .short("d")
                                .long("database")
                                .value_name("FILE")
                                .help("Uses FILE as the database file of the server")
                                .takes_value(true)
                            )
                        
                        )
                        .get_matches();
    
    // see if we are generating a new config file
    if matches.is_present("new") {
        match matches.subcommand_matches("new") {
            Some(new_matches) => {

                // get the number of accesses we need
                let n_accesses = new_matches.value_of("number")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap();
                // see if we need to do the file path too
                if new_matches.is_present("file") {
                    let fname = new_matches.value_of("file").unwrap();
                    config::generate_new(n_accesses, Some(fname.to_string()));
                } else {
                    config::generate_new(n_accesses, None);
                }

                // print that we succeeded
                log(LTYPE::Info, format!("Successfully generated new config with {} access UUIDs", n_accesses));
        
            },
            None => {
                log(LTYPE::Error, format!("Failed to parse new config arguments!"))
            }
        }
        std::process::exit(0);
        
    }

    // parse the config file
    if matches.is_present("run") {
        match matches.subcommand_matches("run") {
            Some(new_matches) => {
                let cfg = new_matches.value_of("config")
                    .unwrap();
                let cfg_file = match config::open_config(cfg.to_string()) {
                    Ok(a) => a,
                    Err(e) => {
                        log(LTYPE::Error, format!("Failed to open config file: {}", e));
                        std::process::exit(1);
                    }
                };

                if new_matches.is_present("database") {
                    let dbase_path = match new_matches.value_of("database") {
                        Some(a) => a,
                        None => panic!("No value provided to argument")
                    };


                    log(LTYPE::Info, format!("Running with DB at path {}", dbase_path));
                    // run the server
                    run_server(cfg_file, Some(dbase_path.to_string())).await

                } else {
                    // run the server
                    run_server(cfg_file, None).await
                }
            },
            None => panic!("Missing crit arg")
        }
    }

    // if we get here we know that no arguments were provided
    log(LTYPE::Error, format!("No arguments provided! Use '--help' to view usage"));
    
}