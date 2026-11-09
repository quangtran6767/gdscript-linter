//! One bounded QuickJS runtime per provider/worker; no module loader or OS APIs.
use rquickjs::{Context, Ctx, Function, Module, Object, Runtime};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
pub const DEFAULT_TIME_LIMIT: Duration = Duration::from_secs(2);

pub struct Sandbox {
    pub context: Context,
    runtime: Runtime,
    deadline: Rc<Cell<Instant>>,
    pub time_limit: Duration,
}

impl Sandbox {
    pub fn new() -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|error| error.to_string())?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        runtime.set_max_stack_size(512 * 1024);
        let deadline = Rc::new(Cell::new(Instant::now() + DEFAULT_TIME_LIMIT));
        let interrupt_deadline = deadline.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            Instant::now() >= interrupt_deadline.get()
        })));
        let context = Context::full(&runtime).map_err(|error| error.to_string())?;
        let sandbox = Self {
            context,
            runtime,
            deadline,
            time_limit: DEFAULT_TIME_LIMIT,
        };
        sandbox.context.with(|ctx| -> Result<(), String> {
            let result = ctx.eval::<(), _>(include_str!("bridge.js"));
            result.map_err(|error| describe_error(&ctx, &error))
        })?;
        Ok(sandbox)
    }

    pub fn begin(&self) {
        self.deadline.set(Instant::now() + self.time_limit);
    }
    pub fn collect_garbage(&self) {
        self.runtime.run_gc();
    }

    pub fn load(&self, index: usize, path: &str, source: &str) -> Result<String, String> {
        self.begin();
        self.context.with(|ctx| {
            let result = (|| -> rquickjs::Result<String> {
                // A logger is present but quiet by default. Rules cannot use
                // stdout and corrupt machine-readable CLI/LSP output.
                ctx.globals().set("console", Object::new(ctx.clone())?)?;
                let console: Object = ctx.globals().get("console")?;
                let verbose = std::env::var_os("GDGUARD_VERBOSE").is_some();
                console.set("log", Function::new(ctx.clone(), move |arguments: rquickjs::function::Rest<rquickjs::Coerced<String>>| {
                    if verbose {
                        let mut parts = Vec::with_capacity(arguments.0.len());
                        for argument in arguments.0 { parts.push(argument.0); }
                        eprintln!("{}", parts.join(" "));
                    }
                })?)?;
                let module = Module::declare(ctx.clone(), path, source)?;
                let (module, promise) = module.eval()?;
                promise.finish::<()>()?;
                let rule: Object = module.get("default")?;
                let rules: rquickjs::Array = ctx.globals().get("__rules")?;
                rules.set(index, rule.clone())?;
                ctx.globals().set("__loadedRule", rule)?;
                ctx.eval("JSON.stringify({meta:__loadedRule.meta, tests:__loadedRule.tests ?? {}, hasCreate:typeof __loadedRule.create === 'function'})")
            })();
            result.map_err(|error| describe_error(&ctx, &error))
        })
    }
}

pub fn describe_error(ctx: &Ctx, error: &rquickjs::Error) -> String {
    if error.is_exception() {
        let caught = ctx.catch();
        if let Some(object) = caught.as_object() {
            if let Some(exception) = rquickjs::Exception::from_object(object.clone()) {
                return format!(
                    "{} (sandbox limit: 64 MB, 2 s per file)",
                    exception
                        .message()
                        .unwrap_or_else(|| "JavaScript exception".to_string())
                );
            }
        }
        return "JavaScript exception (memory limit 64 MB or time limit 2 s exceeded)".to_string();
    }
    error.to_string()
}
