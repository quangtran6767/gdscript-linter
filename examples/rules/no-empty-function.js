/** @type {import("gdguard").Rule} */
export default {
  meta: {id: 'no-empty-function', severity: 'info', description: 'Document or implement empty functions.',
    rationale: 'A function with only pass may be unfinished. An explanatory comment documents an intentional empty callback.'},
  create(context) {
    return {function_definition(node) {
      const body = node.field('body');
      const children = body?.namedChildren ?? [];
      if (children.length === 1 && children[0].type === 'pass_statement') context.report({node: node.field('name') ?? node,
        message: 'Function has an empty body', help: 'Implement the function or add a comment explaining the intentional empty callback.'});
    }};
  },
  tests: {
    valid: ['func f():\n\tprint(1)\n', 'func f():\n\t# Required empty callback.\n\tpass\n'],
    invalid: [{code: 'func f():\n\tpass\n', errors: 1}]
  }
};
