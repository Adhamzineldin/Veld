//! The arguments of a call, in wire form.

use std::collections::BTreeMap;

use serde_json::{Map, Value as Json};
use veld_ir::{SagaCall, SagaValue};

use super::SagaError;
use super::run::Orchestrator;
use crate::eval::{Env, eval};

impl Orchestrator<'_> {
    /// The values the steps have answered so far, by step name: what a later call reads.
    fn step_values(&self) -> BTreeMap<String, Json> {
        self.state
            .steps
            .iter()
            .filter_map(|s| s.value.clone().map(|v| (s.name.clone(), v)))
            .collect()
    }

    /// Evaluates the arguments of `call` against the parameters of the saga and the steps that
    /// have answered, and returns them by the name of the parameter they are for.
    pub(super) fn args_of(
        &self,
        step: &str,
        call: &SagaCall,
    ) -> Result<BTreeMap<String, Json>, SagaError> {
        let mut env = Env::new(self.input.ir);
        env.params = self.input.params.clone();
        env.facts = self.step_values();
        let mut out = BTreeMap::new();
        for a in &call.args {
            let value = wire(&env, &a.value).map_err(|why| SagaError::Argument {
                step: step.to_owned(),
                why,
            })?;
            out.insert(a.name.clone(), value);
        }
        Ok(out)
    }
}

fn wire(env: &Env<'_>, v: &SagaValue) -> Result<Json, String> {
    match v {
        SagaValue::Expr(e) => eval(env, e)
            .map_err(|f| format!("{f:?}"))?
            .to_json()
            .map_err(|e| e.to_string()),
        SagaValue::Object { object } => {
            let mut map = Map::new();
            for f in object {
                map.insert(f.wire.clone(), wire(env, &f.value)?);
            }
            Ok(Json::Object(map))
        }
    }
}
