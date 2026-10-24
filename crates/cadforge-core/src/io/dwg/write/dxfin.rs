//! The DXF that [`crate::io::write_dxf`] produces, split into its records: what the DWG writer translates.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Rec {
    pub ty: String,
    pub g: Vec<(i32, String)>,
}

impl Rec {
    pub fn get(&self, c: i32) -> Option<&str> {
        self.g.iter().find(|(k, _)| *k == c).map(|(_, v)| v.as_str())
    }
    pub fn s(&self, c: i32) -> &str {
        self.get(c).unwrap_or("")
    }
    pub fn fd(&self, c: i32, d: f64) -> f64 {
        self.get(c).and_then(|v| v.trim().parse().ok()).unwrap_or(d)
    }
    pub fn f(&self, c: i32) -> f64 {
        self.fd(c, 0.0)
    }
    pub fn i(&self, c: i32, d: i64) -> i64 {
        self.get(c).and_then(|v| v.trim().parse().ok()).unwrap_or(d)
    }
    /// The point in groups c, c+10, c+20.
    pub fn p(&self, c: i32) -> [f64; 3] {
        [self.f(c), self.f(c + 10), self.f(c + 20)]
    }
    pub fn p2(&self, c: i32) -> [f64; 2] {
        [self.f(c), self.f(c + 10)]
    }
    pub fn all(&self, c: i32) -> Vec<&str> {
        self.g.iter().filter(|(k, _)| *k == c).map(|(_, v)| v.as_str()).collect()
    }
    pub fn handle(&self) -> u64 {
        hex(self.get(5).or(self.get(105)).unwrap_or("0"))
    }
    /// The owner (the 330 outside any {ACAD_REACTORS ...} bracket).
    pub fn owner(&self) -> u64 {
        let mut inside = false;
        for (c, v) in &self.g {
            match *c {
                102 => inside = v.starts_with('{'),
                330 if !inside => return hex(v),
                _ => {}
            }
        }
        0
    }
    /// The handles inside {ACAD_REACTORS ...}.
    pub fn reactors(&self) -> Vec<u64> {
        let mut inside = false;
        let mut out = vec![];
        for (c, v) in &self.g {
            match *c {
                102 => inside = v.starts_with("{ACAD_REACTORS"),
                330 if inside => out.push(hex(v)),
                _ => {}
            }
        }
        out
    }
}

pub fn hex(s: &str) -> u64 {
    u64::from_str_radix(s.trim(), 16).unwrap_or(0)
}

pub struct Block {
    pub begin: Rec,
    pub ents: Vec<Rec>,
    pub end: Rec,
}

#[derive(Default)]
pub struct Dxf {
    pub header: HashMap<String, Rec>,
    /// TABLE record and its entries, by table name.
    pub tables: Vec<(Rec, Vec<Rec>)>,
    pub blocks: Vec<Block>,
    pub entities: Vec<Rec>,
    pub objects: Vec<Rec>,
}

impl Dxf {
    pub fn table(&self, name: &str) -> Option<&(Rec, Vec<Rec>)> {
        self.tables.iter().find(|(t, _)| t.s(2) == name)
    }
}

pub fn parse(text: &str) -> Dxf {
    let lines: Vec<&str> = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let mut recs: Vec<Rec> = vec![];
    let mut i = 0;
    while i + 1 < lines.len() {
        if let Ok(code) = lines[i].trim().parse::<i32>() {
            let v = lines[i + 1].to_string();
            if code == 0 {
                recs.push(Rec { ty: v.trim().to_string(), g: vec![] });
            } else if let Some(r) = recs.last_mut() {
                r.g.push((code, v));
            }
        }
        i += 2;
    }
    let mut d = Dxf::default();
    let mut section = String::new();
    let mut it = recs.into_iter().peekable();
    while let Some(r) = it.next() {
        match r.ty.as_str() {
            "SECTION" => {
                section = r.s(2).to_string();
                if section == "HEADER" {
                    // header variables: 9 $NAME followed by their groups
                    let mut name = String::new();
                    for (c, v) in r.g.into_iter().skip(1) {
                        if c == 9 {
                            name = v.trim().to_string();
                            d.header.insert(name.clone(), Rec { ty: name.clone(), g: vec![] });
                        } else if let Some(h) = d.header.get_mut(&name) {
                            h.g.push((c, v));
                        }
                    }
                }
            }
            "ENDSEC" | "EOF" => section.clear(),
            "TABLE" if section == "TABLES" => {
                let mut entries = vec![];
                while let Some(e) = it.next_if(|e| e.ty != "ENDTAB") {
                    entries.push(e);
                }
                it.next();
                d.tables.push((r, entries));
            }
            "BLOCK" if section == "BLOCKS" => {
                let mut ents = vec![];
                while let Some(e) = it.next_if(|e| e.ty != "ENDBLK") {
                    ents.push(e);
                }
                let end = it.next().unwrap_or_default();
                d.blocks.push(Block { begin: r, ents, end });
            }
            _ if section == "ENTITIES" => d.entities.push(r),
            _ if section == "OBJECTS" => d.objects.push(r),
            _ => {}
        }
    }
    d
}
