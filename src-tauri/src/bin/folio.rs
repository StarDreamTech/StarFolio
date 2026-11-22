use anyhow::{bail, ensure, Result};
use starfolio_lib::{
    capture, default_root, import, import_collection, import_local_image, list, recheck,
    restore_x_cover, verify, LocalImage,
};
use std::path::Path;
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    let root = std::env::var_os("STARFOLIO_LIBRARY")
        .map(std::path::PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_root)?;
    match a.as_slice() {
        [cmd, id] if cmd == "translation-source" => {
            println!("{}", serde_json::to_string_pretty(&starfolio_lib::translation::source(&root, id)?)?);
            return Ok(());
        }
        [cmd, id, input] if cmd == "add-chinese" => {
            let saved = starfolio_lib::translation::add_chinese(&root, id, Path::new(input))?;
            println!("{}", if saved { "Chinese translation saved" } else { "Chinese translation already present" });
            return Ok(());
        }
        [cmd, image, spec] if cmd == "import-image" => {
            ensure!(
                std::fs::metadata(spec)?.len() <= 256 << 10,
                "图片说明文件过大"
            );
            let meta: LocalImage = serde_json::from_slice(&std::fs::read(spec)?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&import_local_image(&root, Path::new(image), meta)?)?
            );
            return Ok(());
        }
        [cmd, id, url, image] if cmd == "restore-x-cover" => {
            match restore_x_cover(&root, id, url, Path::new(image))? {
                true => println!("Cover restored and verified"),
                false => println!("Cover already present and verified"),
            }
            return Ok(());
        }
        _ => {}
    }
    match a.as_slice(){[cmd,dir] if cmd=="import-avatars"=>{let(users,articles)=starfolio_lib::avatars::import(&root,std::path::Path::new(dir))?;println!("{users} author mappings saved; {articles} reader pages updated");},[cmd,dir] if cmd=="import-collection"=>println!("{} records imported or already present",import_collection(&root,std::path::Path::new(dir))?),[cmd,id] if cmd=="review"=>println!("{}",serde_json::to_string_pretty(&recheck(&root,id)?)?),[cmd,url,file] if cmd=="import"=>{let md=std::fs::metadata(file)?;anyhow::ensure!(md.len()<=16<<20,"file too large");let s=import(&root,url,&std::fs::read_to_string(file)?)?;println!("{}",serde_json::to_string_pretty(&s)?);},[cmd,url] if cmd=="save"=>{let s=capture(&root,url)?;println!("{}",serde_json::to_string_pretty(&s)?);println!("{}",root.join("snapshots").join(s.id).join("index.html").display());},[cmd] if cmd=="list"=>println!("{}",serde_json::to_string_pretty(&list(&root)?)?),[cmd,id] if cmd=="verify"=>println!("verified {} entries and image hashes",verify(&root,id)?),_=>bail!("folio translation-source <snapshot-id> | add-chinese <snapshot-id> <translation-json> | import-image <local-image> <metadata-json> | restore-x-cover <snapshot-id> <browser-cover-url> <local-image> | import-avatars <avatar-store> | import-collection <folder> | save <url> | import <source-url> <html-or-json-file> | list | review <snapshot-id> | verify <snapshot-id>")}
    Ok(())
}
