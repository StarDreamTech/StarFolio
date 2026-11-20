use anyhow::{bail, Result};
use starfolio_lib::{capture, default_root, import, list, recheck, verify};
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    let root = default_root()?;
    match a.as_slice(){[cmd,id] if cmd=="review"=>println!("{}",serde_json::to_string_pretty(&recheck(&root,id)?)?),[cmd,url,file] if cmd=="import"=>{let md=std::fs::metadata(file)?;anyhow::ensure!(md.len()<=16<<20,"file too large");let s=import(&root,url,&std::fs::read_to_string(file)?)?;println!("{}",serde_json::to_string_pretty(&s)?);},[cmd,url] if cmd=="save"=>{let s=capture(&root,url)?;println!("{}",serde_json::to_string_pretty(&s)?);println!("{}",root.join("snapshots").join(s.id).join("index.html").display());},[cmd] if cmd=="list"=>println!("{}",serde_json::to_string_pretty(&list(&root)?)?),[cmd,id] if cmd=="verify"=>println!("verified {} entries and image hashes",verify(&root,id)?),_=>bail!("folio save <url> | import <source-url> <html-or-json-file> | list | review <snapshot-id> | verify <snapshot-id>")}
    Ok(())
}
