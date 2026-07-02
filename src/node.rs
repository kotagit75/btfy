use std::io;

use openssl::error::ErrorStack;

use crate::{
    blockchain::chain::Chain,
    util::key::{SK, generate_pk_and_sk},
};

const NODE_KEY_BITS: u32 = 512;

const NODE_DIR_PATH: &str = "node";
const NODE_GITIGNORE_PATH: &str = "node/.gitignore";
const NODE_KEY_PATH: &str = "node/key.der";
const NODE_CHAIN_PATH: &str = "node/chain";

fn create_node_dir() -> Result<(), ()> {
    info!("creating node directory");
    std::fs::create_dir(NODE_DIR_PATH)
        .inspect_err(|e| error!("failed to create the node directory: {}", e))
        .map_err(|_| ())?;

    Ok(())
}

fn create_gitignore() -> Result<(), ()> {
    info!("creating gitignore");
    std::fs::write(NODE_GITIGNORE_PATH, format!("{}\n", NODE_KEY_PATH))
        .inspect_err(|e| error!("failed to create the gitignore file: {}", e))
        .map_err(|_| ())?;

    Ok(())
}

pub fn load_or_generate_key() -> Result<SK, ()> {
    if std::fs::metadata(NODE_DIR_PATH).is_err() {
        create_node_dir()?;
    }
    if std::fs::metadata(NODE_GITIGNORE_PATH).is_err() {
        create_gitignore()?;
    }

    if std::fs::metadata(NODE_KEY_PATH).is_ok() {
        info!("reading node key");
        read_key().map_err(|_| {
            error!("failed to read node key");
        })
    } else {
        info!("generating node key");
        match generate_key() {
            Ok(sk) => {
                save_key(&sk).map_err(|err| {
                    error!("failed to save node key: {}", err);
                    ()
                })?;
                Ok(sk)
            }
            Err(_) => Err(()),
        }
    }
}

pub fn generate_key() -> Result<SK, ErrorStack> {
    generate_pk_and_sk(NODE_KEY_BITS).map(|(_, sk)| sk)
}

pub fn read_key() -> Result<SK, io::Error> {
    std::fs::read_to_string(NODE_KEY_PATH).map(|der| SK { der: der })
}

pub fn save_key(sk: &SK) -> Result<(), io::Error> {
    std::fs::write(NODE_KEY_PATH, &sk.der)
}

pub fn load_or_generate_chain() -> Result<Chain, io::Error> {
    if std::fs::metadata(NODE_CHAIN_PATH).is_err() {
        info!("generating chain");
        let chain = Chain::new();
        save_chain(&chain).inspect_err(|e| {
            error!("failed to save chain: {}", e);
        })?;
        return Ok(chain);
    }
    load_chain()
}
pub fn load_chain() -> Result<Chain, io::Error> {
    std::fs::read(NODE_CHAIN_PATH).and_then(|s| {
        rmp_serde::from_slice(&s).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    })
}
pub fn save_chain(chain: &Chain) -> Result<(), io::Error> {
    let buf = rmp_serde::to_vec(chain).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    std::fs::write(NODE_CHAIN_PATH, buf)
}
