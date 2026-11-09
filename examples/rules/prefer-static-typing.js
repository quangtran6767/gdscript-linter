/** @type {import("gdguard").Rule<{allow_inferred?:boolean}>} */
export default {
  meta:{id:'prefer-static-typing',description:'Give variables explicit or inferred types.',rationale:'Typed declarations document intent and improve editor checks.'},
  create(context) {return {variable_statement(node) {
    const type=node.field('type');
    if (!type || (type.type === 'inferred_type' && context.options.allow_inferred === false))
      context.report({node:node.field('name') ?? node,message:'Variable needs a type annotation',help:'Use : Type, or := when inferred types are allowed.'});
  }};},
  tests:{valid:['var value: int = 1\n','var value := 1\n'],invalid:[{code:'var value = 1\n',errors:1}]}
};
