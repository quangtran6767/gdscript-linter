/** @type {import("gdguard").Rule} */
export default {
  meta:{id:'no-autoload-mutation-outside-owner',description:'Keep autoload mutation in its owning script.',rationale:'An autoload method makes state changes explicit and keeps invariants in one place.'},
  create(context) {
    /** @param {import("gdguard").Node} node */
    function check(node) {
      if (!context.project) return;
      const left=node.field('left');
      if (left?.type !== 'attribute') return;
      const owner=left.namedChildren[0]?.text;
      const path=context.project.autoloads[owner ?? ''];
      if (!path) return;
      const filename=context.filename.replaceAll('\\','/');
      const ownerPath = context.project.root?.replaceAll('\\','/') + '/' + path;
      if (filename === path || filename === ownerPath) return;
      context.report({node:left,message:`Mutate ${owner} through its owning script`,help:'Add a method on the autoload and call it instead of assigning its member.'});
    }
    return {assignment:check,augmented_assignment:check};
  },
  // Project-specific positive cases are in tests/js_rules and C5 e2e.
  tests:{valid:['func f():\n\tGame.score = 1\n','var local := 1\n']}
};
