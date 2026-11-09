/** @type {import("gdguard").Rule<{max?:number}>} */
export default {
  meta: {id: 'max-function-params', severity: 'warning', description: 'Keep function parameter lists short.',
    rationale: 'Long parameter lists obscure responsibilities. Group related values into a Resource.'},
  create(context) {
    const max = context.options.max ?? 5;
    return {function_definition(node) {
      const params = node.field('parameters')?.namedChildren ?? [];
      if (params.length > max) context.report({node: node.field('name') ?? node,
        message: `Function has ${params.length} parameters (max ${max})`, help: 'Group related parameters into a Resource or Dictionary.'});
    }};
  },
  tests: {
    valid: ['func f(a, b):\n\treturn a + b\n', 'func f():\n\tpass\n'],
    invalid: [{code: 'func f(a, b, c, d, e, f):\n\tpass\n', errors: 1}]
  }
};
