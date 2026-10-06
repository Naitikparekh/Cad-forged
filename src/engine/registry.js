// Command registry: definitions, aliases (acad.pgp style, last definition wins) and synonyms.

export const registry = {
  commands: new Map(),     // NAME -> definition
  aliases: new Map(),      // ALIAS -> NAME
  userAliases: new Map(),  // user overrides (win over defaults)
  synonyms: new Map(),     // natural word -> NAME

  /**
   * @param {{name:string, aliases?:string[], run:Function, transparent?:boolean, pickfirst?:boolean, group?:string,
   *          title?:string, desc?:string, icon?:string, repeatable?:boolean}} def
   */
  define(def) {
    const name = def.name.toUpperCase();
    def = { repeatable: true, ...def, name };
    this.commands.set(name, def);
    for (const a of def.aliases || []) this.aliases.set(a.toUpperCase(), name);
    return def;
  },

  alias(alias, name) { this.aliases.set(alias.toUpperCase(), name.toUpperCase()); },

  /** Resolve typed text to a command definition (tries exact name, user alias, alias, synonym). */
  find(text) {
    let t = String(text).trim().toUpperCase();
    if (!t) return null;
    const dash = t.startsWith('-');                         // -LAYER etc: command-line variants
    const key = this.userAliases.get(t) || this.aliases.get(t) || (this.commands.has(t) ? t : null) || this.synonyms.get(t);
    if (key) return this.commands.get(key) || null;
    if (dash) return this.commands.get(t) || null;
    return null;
  },

  names() { return [...this.commands.keys()].sort(); },

  /** Typed-input suggestions: commands first by prefix, then mid-string matches. */
  suggest(text, { midString = true, extra = [], limit = 12 } = {}) {
    const t = text.trim().toUpperCase();
    if (!t) return [];
    const pool = [...this.commands.keys(), ...extra];
    const starts = pool.filter(n => n.startsWith(t)).sort((a, b) => a.length - b.length || a.localeCompare(b));
    const mid = midString ? pool.filter(n => !n.startsWith(t) && n.includes(t)).sort() : [];
    const target = this.userAliases.get(t) || this.aliases.get(t) || (this.commands.has(t) ? t : null);   // what Enter would run
    return [...new Set([...(target ? [target] : []), ...starts, ...mid])].slice(0, limit);
  },
};
