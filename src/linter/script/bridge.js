// Node wrappers request only the property being read from the native tree.
// No syntax-tree object graph is copied into the JS heap.
globalThis.__rules = [];
globalThis.__prepare = function(index, info, options) {
  const rule = __rules[index];
  const nodes = new Map();
  function node(path) {
    const key = path.join('/');
    if (nodes.has(key)) return nodes.get(key);
    const value = Object.freeze({
      get type() { return JSON.parse(__node(path, 'type', '')); },
      get text() { return JSON.parse(__node(path, 'text', '')); },
      get range() { return JSON.parse(__node(path, 'range', '')); },
      get parent() { return path.length ? node(path.slice(0, -1)) : null; },
      get children() { return JSON.parse(__node(path, 'children', '')).map(node); },
      get namedChildren() { return JSON.parse(__node(path, 'namedChildren', '')).map(node); },
      field(name) { const child = JSON.parse(__node(path, 'field', name)); return child === null ? null : node(child); }
    });
    nodes.set(key, value);
    return value;
  }
  const range = target => target.range ?? target;
  const fix = Object.freeze({
    replace(target, replacement) { return {range: range(target), replacement}; },
    insertBefore(target, replacement) { const p = range(target).start; return {range: {start:p, end:p}, replacement}; },
    insertAfter(target, replacement) { const p = range(target).end; return {range: {start:p, end:p}, replacement}; },
    remove(target) { return {range: range(target), replacement:''}; }
  });
  const issues = [];
  const semantic = info.hasSemantic ? Object.freeze({
    get symbols() { return JSON.parse(__semantic('symbols', 0, '')); },
    scopeAt(byte) { return JSON.parse(__semantic('scopeAt', byte, '')); },
    lookup(scope, name) { return JSON.parse(__semantic('lookup', scope, name)); },
    referencesTo(symbol) { return JSON.parse(__semantic('referencesTo', symbol, '')); }
  }) : null;
  const project = info.hasProject ? Object.freeze({
    get root() { return JSON.parse(__project('root', '', '')); },
    get classNames() { return JSON.parse(__project('classNames', '', '')); },
    get autoloads() { return JSON.parse(__project('autoloads', '', '')); },
    hasMember(name, member) { return JSON.parse(__project('hasMember', name, member)); }
  }) : null;
  const context = Object.freeze({
    filename: info.filename, source: info.source, options: Object.freeze(options), fix, semantic, project,
    report(descriptor) {
      if (!descriptor || typeof descriptor.message !== 'string' || (!!descriptor.node === !!descriptor.range))
        throw new TypeError('report needs message and exactly one of node or range');
      let edits = typeof descriptor.fix === 'function' ? descriptor.fix(fix) : descriptor.fix;
      if (edits && !Array.isArray(edits)) edits = [edits];
      issues.push({range: descriptor.node ? descriptor.node.range : descriptor.range,
        message: descriptor.message, help: descriptor.help ?? null, edits: edits ?? [],
        safety: descriptor.fixSafety ?? rule.meta.fixSafety ?? 'suggestion',
        description: descriptor.fixDescription ?? `Fix ${rule.meta.id}`});
    }
  });
  const visitors = rule.create(context);
  if (!visitors || typeof visitors !== 'object') throw new TypeError('create must return a visitor map');
  for (const [kind, handler] of Object.entries(visitors)) {
    if (typeof handler !== 'function') throw new TypeError(`visitor ${kind} must be a function`);
  }
  globalThis.__dispatch = function(kind, path) { visitors[kind](node(path)); };
  globalThis.__finish = function() { return JSON.stringify(issues); };
  return Object.keys(visitors);
};
