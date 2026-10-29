//! Routes a parsed command line to its implementation.

use super::args::{Cli, Cmd};
use crate::commands::{audit, files, read, setup, verify, write};
use crate::output::Output;
use anyhow::{Result, bail};

pub fn dispatch(cli: &Cli) -> Result<Output> {
    let g = &cli.global;
    match &cli.cmd {
        Cmd::Context(a) => read::context::run(g, a),
        Cmd::Find(a) => read::find::run(g, a),
        Cmd::Show(a) => read::show::run(g, a),
        Cmd::Resolve(a) => read::resolve::run(g, a),
        Cmd::Graph(a) => read::graph::run(g, a),
        Cmd::Owners(a) => read::owners::run(g, a),
        Cmd::Assets(a) => read::assets::run(g, a),
        Cmd::Log(a) => read::log::run(g, a),
        Cmd::New(a) => write::new::run(g, a),
        Cmd::Set(a) => write::set::run(g, a),
        Cmd::Edit(a) => write::edit::run(g, a),
        Cmd::Mv(a) => write::mv::run(g, a),
        Cmd::Fix(a) => write::fix::run(g, a),
        Cmd::Sync(a) => write::sync::run(g, a),
        Cmd::Init(a) => write::init::run(g, a),
        Cmd::Import(a) => write::import::run(g, a),
        Cmd::Attach(a) => files::attach::run(g, a),
        Cmd::Fetch(a) => files::fetch::run(g, a),
        Cmd::Files(a) => files::list::run(g, a),
        Cmd::Check(a) => verify::check::run(g, a),
        Cmd::Verify(a) => verify::verify::run(g, a),
        Cmd::Guard(a) => verify::guard::run(g, a),
        Cmd::Census(a) => verify::census::run(g, a),
        Cmd::Doctor => verify::doctor::run(g),
        Cmd::Audit(a) => audit::run(g, a),
        Cmd::Explain(a) => setup::explain::run(g, a),
        Cmd::Schema(a) => setup::schema::run(g, a),
        Cmd::Help(a) => setup::help::run(g, a),
        Cmd::Agents(a) => setup::agents::run(g, a),
        Cmd::Hook(a) => setup::hook::run(g, a),
        Cmd::Plugins(a) => setup::plugins::run(g, a),
        Cmd::Export(a) => setup::export::run(g, a),
        Cmd::Mcp => bail!("`spec mcp` serves over stdio and is started by an MCP client, not inside another command"),
    }
}
