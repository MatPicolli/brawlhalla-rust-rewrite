//! Lista de ossos (`<BoneTypes>` em `Init.swz`). O índice de uma parte num `.anm` aponta para esta lista,
//! que no jogo começa com a entrada `UNKNOWN` (índice 0): o osso `n` do XML tem índice `n + 1`.

use anyhow::{bail, Result};

pub fn bone_names(init_sections: &[Vec<u8>]) -> Result<Vec<String>> {
    for s in init_sections {
        if !s.starts_with(b"<BoneTypes>") {
            continue;
        }
        let text = String::from_utf8_lossy(s);
        let doc = roxmltree::Document::parse(&text)?;
        let mut names = vec!["UNKNOWN".to_string()];
        names.extend(
            doc.root_element()
                .children()
                .filter(|n| n.has_tag_name("Bone"))
                .map(|n| n.text().unwrap_or_default().trim().to_string()),
        );
        return Ok(names);
    }
    bail!("<BoneTypes> não encontrado no Init.swz")
}
