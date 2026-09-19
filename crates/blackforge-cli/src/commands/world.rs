use std::path::Path;

use anyhow::Result;
use blackforge_core::world::{Portal, PortalKind, portals as read_portals, unpaired};

use crate::ui::print_table;

const NO_NAME: &str = "<no name>";

fn shown(name: &str) -> &str {
    if name.is_empty() { NO_NAME } else { name }
}

fn row(portal: &Portal) -> Vec<String> {
    let kind = match portal.kind {
        PortalKind::Wood => "wood",
        PortalKind::Stone => "stone",
        PortalKind::Other => "other",
    };
    vec![
        shown(&portal.name).to_owned(),
        kind.to_owned(),
        format!("{:.0} {:.0}", portal.x, portal.z),
    ]
}

pub async fn portals(world: &Path, only_unpaired: bool) -> Result<()> {
    let portals = read_portals(world).await?;
    if only_unpaired {
        for name in unpaired(&portals) {
            println!("{}", shown(name));
        }
        return Ok(());
    }
    let rows: Vec<Vec<String>> = portals.iter().map(row).collect();
    print_table(&rows);
    println!(
        "{} portals, {} without a pair",
        portals.len(),
        unpaired(&portals).len()
    );
    Ok(())
}
