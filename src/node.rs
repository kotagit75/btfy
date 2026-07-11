use std::io;

use crate::{
    blockchain::chain::Chain,
    util::key::{SK, generate_sk},
};

const NODE_KEY_BITS: usize = 512;

const NODE_DIR_PATH: &str = "node";
const NODE_GITIGNORE_PATH: &str = "node/.gitignore";
const NODE_KEY_PATH: &str = "node/key.der";
const NODE_CHAIN_PATH: &str = "node/chain";

fn create_node_dir() -> Result<(), io::Error> {
    std::fs::create_dir(NODE_DIR_PATH)?;
    Ok(())
}

fn create_gitignore() -> Result<(), io::Error> {
    std::fs::write(NODE_GITIGNORE_PATH, format!("{}\n", NODE_KEY_PATH))?;
    Ok(())
}

pub fn load_or_generate_key() -> Result<SK, io::Error> {
    info!("create node directory");
    if std::fs::metadata(NODE_DIR_PATH).is_err() {
        create_node_dir()
            .inspect(|err| error!("failed to create the node directory: {:?}", err))?;
    }
    info!("create gitignore");
    if std::fs::metadata(NODE_GITIGNORE_PATH).is_err() {
        create_gitignore()
            .inspect(|err| error!("failed to create the gitignore file: {:?}", err))?;
    }

    if std::fs::metadata(NODE_KEY_PATH).is_ok() {
        info!("read node key");
        read_key().inspect_err(|err| {
            error!("failed to read node key: {}", err);
        })
    } else {
        info!("generate node key");
        let sk = generate_key();
        save_key(&sk).inspect_err(|err| {
            error!("failed to save node key: {}", err);
        })?;
        Ok(sk)
    }
}

pub fn generate_key() -> SK {
    generate_sk(NODE_KEY_BITS)
}

pub fn read_key() -> Result<SK, io::Error> {
    std::fs::read_to_string(NODE_KEY_PATH).map(|der| SK { der })
}

pub fn save_key(sk: &SK) -> Result<(), io::Error> {
    std::fs::write(NODE_KEY_PATH, &sk.der)
}

pub fn load_or_generate_chain() -> Result<Chain, io::Error> {
    if std::fs::metadata(NODE_CHAIN_PATH).is_err() {
        info!("generate chain");
        let chain = Chain::new();
        save_chain(&chain).inspect_err(|e| {
            error!("failed to save chain: {}", e);
        })?;
        return Ok(chain);
    }
    load_chain()
}
pub fn load_chain() -> Result<Chain, io::Error> {
    std::fs::read(NODE_CHAIN_PATH).and_then(|s| rmp_serde::from_slice(&s).map_err(io::Error::other))
}
pub fn save_chain(chain: &Chain) -> Result<(), io::Error> {
    let buf = rmp_serde::to_vec(chain).map_err(io::Error::other)?;
    std::fs::write(NODE_CHAIN_PATH, buf)
}
