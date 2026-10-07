//! Default command aliases (the acad.pgp set that CadForge supports) and natural-language synonyms
//! (port of `src/commands/aliases.js`). Aliases for unsupported commands are skipped.
use crate::registry::Registry;

pub const DEFAULT_ALIASES: &[(&str, &str)] = &[
    ("AA", "AREA"), ("AL", "ALIGN"), ("AR", "ARRAY"), ("B", "BLOCK"), ("BH", "HATCH"), ("BO", "BOUNDARY"), ("BR", "BREAK"), ("C", "CIRCLE"),
    ("CH", "PROPERTIES"), ("CHA", "CHAMFER"), ("COL", "COLOR"), ("CO", "COPY"), ("CP", "COPY"), ("D", "DIMSTYLE"), ("DAL", "DIMALIGNED"),
    ("DAN", "DIMANGULAR"), ("DBA", "DIMBASELINE"), ("DCO", "DIMCONTINUE"), ("DDI", "DIMDIAMETER"), ("DI", "DIST"), ("DIV", "DIVIDE"),
    ("DLI", "DIMLINEAR"), ("DO", "DONUT"), ("DR", "DRAWORDER"), ("DRA", "DIMRADIUS"), ("DT", "TEXT"), ("E", "ERASE"), ("ED", "TEXTEDIT"),
    ("EL", "ELLIPSE"), ("EX", "EXTEND"), ("EXIT", "QUIT"), ("EXP", "EXPORT"), ("EXT", "EXTRUDE"), ("F", "FILLET"), ("G", "GROUP"),
    ("H", "HATCH"), ("HE", "HATCHEDIT"), ("I", "INSERT"), ("IN", "INTERSECT"), ("J", "JOIN"), ("L", "LINE"), ("LA", "LAYER"),
    ("LE", "MLEADER"), ("LEN", "LENGTHEN"), ("LI", "LIST"), ("LT", "LINETYPE"), ("LTS", "LTSCALE"), ("LW", "LWEIGHT"), ("M", "MOVE"),
    ("MA", "MATCHPROP"), ("ME", "MEASURE"), ("MI", "MIRROR"), ("MO", "PROPERTIES"), ("MT", "MTEXT"), ("O", "OFFSET"), ("OP", "OPTIONS"),
    ("OS", "OSNAP"), ("P", "PAN"), ("PE", "PEDIT"), ("PL", "PLINE"), ("PO", "POINT"), ("POL", "POLYGON"), ("PR", "PROPERTIES"),
    ("PU", "PURGE"), ("R", "REDRAW"), ("RE", "REGEN"), ("REA", "REGENALL"), ("REC", "RECTANG"), ("REN", "-RENAME"), ("RO", "ROTATE"),
    ("S", "STRETCH"), ("SC", "SCALE"), ("SCR", "SCRIPT"), ("SET", "SETVAR"), ("SN", "SNAP"), ("SPL", "SPLINE"), ("ST", "STYLE"),
    ("SU", "SUBTRACT"), ("T", "MTEXT"), ("TP", "TOOLPALETTES"), ("TR", "TRIM"), ("UN", "UNITS"), ("UNI", "UNION"), ("V", "-VIEW"),
    ("W", "WBLOCK"), ("X", "EXPLODE"), ("XL", "XLINE"), ("Z", "ZOOM"),
    // dimension / drafting extras
    ("QP", "QUICKPROPERTIES"), ("QSAVE", "QSAVE"), ("3DO", "3DORBIT"), ("BEDIT", "BEDIT"), ("RAY", "RAY"), ("ID", "ID"), ("REV", "REVOLVE"),
    ("WE", "WEDGE"), ("CYL", "CYLINDER"), ("SPH", "SPHERE"), ("SUB", "SUBTRACT"),
];

pub const DEFAULT_SYNONYMS: &[(&str, &str)] = &[
    ("BEVEL", "CHAMFER"), ("SYMBOL", "BLOCK"), ("RECTANGLE", "RECTANG"), ("DELETE", "ERASE"), ("CUT", "TRIM"), ("LABEL", "MTEXT"),
    ("ROUND", "FILLET"), ("DUPLICATE", "COPY"), ("STRETCH", "STRETCH"), ("PARALLEL", "OFFSET"), ("LAYERS", "LAYER"),
    ("EXTRUSION", "EXTRUDE"), ("GRIDON", "GRID"), ("MEASUREMENT", "DIST"), ("LEADER", "MLEADER"), ("SQUARE", "RECTANG"),
    ("PRINT", "PLOT"), ("SAVEAS", "SAVEAS"),
];

pub fn install(r: &mut Registry) {
    for (a, n) in DEFAULT_ALIASES {
        if r.has_command(n) && !r.has_alias(a) && !r.has_command(a) {
            r.alias(a, n);
        }
    }
    for (w, n) in DEFAULT_SYNONYMS {
        if r.has_command(n) && !r.has_command(w) {
            r.synonyms.insert(w.to_string(), n.to_string());
        }
    }
}
