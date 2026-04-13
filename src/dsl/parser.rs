use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct ActionCall {
    pub name: String,
    pub params: HashMap<String, String>,
}

impl ActionCall {

    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let raw = raw.trim();
        if let Some(paren) = raw.find('(') {
            let name = raw[..paren].trim().to_string();
            let inner = raw[paren + 1..].trim_end_matches(')').trim();
            let mut params = HashMap::new();
            for part in inner.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let (k, v) = part
                    .split_once('=')
                    .ok_or_else(|| anyhow::anyhow!("paramètre invalide: {}", part))?;
                params.insert(k.trim().to_string(), v.trim().to_string());
            }
            Ok(ActionCall { name, params })
        } else {
            Ok(ActionCall {
                name: raw.to_string(),
                params: HashMap::new(),
            })
        }
    }

    pub fn to_string_repr(&self) -> String {
        if self.params.is_empty() {
            self.name.clone()
        } else {
            let args: Vec<String> = self
                .params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            format!("{}({})", self.name, args.join(", "))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StepGroup {
    pub actions: Vec<ActionCall>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecipeStep {
    pub group: StepGroup,

    pub repeat: u32,
}

impl RecipeStep {

    pub fn all_actions(&self) -> Vec<ActionCall> {
        let mut result = Vec::new();
        for _ in 0..self.repeat {
            result.extend(self.group.actions.clone());
        }
        result
    }

}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: String,
    pub steps: Vec<RecipeStep>,
}

impl Recipe {

    pub fn all_actions(&self) -> Vec<ActionCall> {
        self.steps.iter().flat_map(|s| s.all_actions()).collect()
    }

    pub fn required_capabilities(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        self.all_actions()
            .into_iter()
            .map(|a| a.name)
            .filter(|n| seen.insert(n.clone()))
            .collect()
    }

    pub fn missing_actions(&self, available: &[String]) -> Vec<String> {
        self.required_capabilities()
            .into_iter()
            .filter(|cap| !available.contains(cap))
            .collect()
    }
}

pub fn parse_recipes(input: &str) -> anyhow::Result<HashMap<String, Recipe>> {
    let mut recipes = HashMap::new();
    let mut current_name: Option<String> = None;
    let mut current_steps: Vec<RecipeStep> = Vec::new();

    for (lineno, line) in input.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() {
            if let Some(name) = current_name.take() {
                recipes.insert(name.clone(), Recipe { name, steps: current_steps.clone() });
                current_steps.clear();
            }
            continue;
        }

        if line.starts_with('#') {
            continue;
        }

        if let Some(name) = line.strip_suffix('=').map(str::trim) {
            if let Some(prev) = current_name.take() {
                recipes.insert(prev.clone(), Recipe { name: prev, steps: current_steps.clone() });
                current_steps.clear();
            }
            current_name = Some(name.to_string());
            continue;
        }

        let step_raw = line.strip_prefix("->").unwrap_or(line).trim();
        if step_raw.is_empty() {
            continue;
        }

        let (step_part, repeat) = if let Some(pos) = step_raw.rfind('^') {
            let n: u32 = step_raw[pos + 1..]
                .parse()
                .map_err(|_| anyhow::anyhow!("ligne {}: répétition invalide '{}'", lineno + 1, step_raw))?;
            (&step_raw[..pos], n)
        } else {
            (step_raw, 1)
        };

        let group = if step_part.starts_with('[') && step_part.ends_with(']') {
            let inner = &step_part[1..step_part.len() - 1];
            let actions = inner
                .split(',')
                .map(|s| ActionCall::parse(s.trim()))
                .collect::<anyhow::Result<Vec<_>>>()?;
            StepGroup { actions }
        } else {
            StepGroup {
                actions: vec![ActionCall::parse(step_part)?],
            }
        };

        if current_name.is_some() {
            current_steps.push(RecipeStep { group, repeat });
        }
    }

    if let Some(name) = current_name.take() {
        recipes.insert(name.clone(), Recipe { name, steps: current_steps });
    }

    Ok(recipes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_call_simple() {
        let a = ActionCall::parse("MakeDough").unwrap();
        assert_eq!(a.name, "MakeDough");
        assert!(a.params.is_empty());
    }

    #[test]
    fn test_action_call_with_params() {
        let a = ActionCall::parse("AddCheese(amount=2)").unwrap();
        assert_eq!(a.name, "AddCheese");
        assert_eq!(a.params["amount"], "2");
    }

    #[test]
    fn test_parse_simple_recipe() {
        let input = "Pepperoni =\n    MakeDough\n    -> Bake(duration=6)\n";
        let recipes = parse_recipes(input).unwrap();
        let r = &recipes["Pepperoni"];
        assert_eq!(r.steps.len(), 2);
    }

    #[test]
    fn test_parse_parallel_steps() {
        let input = "Test =\n    [ActionA, ActionB]\n";
        let recipes = parse_recipes(input).unwrap();
        let r = &recipes["Test"];
        assert_eq!(r.steps[0].group.actions.len(), 2);
    }

    #[test]
    fn test_parse_repeat() {
        let input = "Test =\n    AddCheese(amount=1)^4\n";
        let recipes = parse_recipes(input).unwrap();
        let r = &recipes["Test"];
        assert_eq!(r.steps[0].repeat, 4);
        assert_eq!(r.steps[0].all_actions().len(), 4);
    }

    #[test]
    fn test_missing_actions() {
        let input = "Margherita =\n    MakeDough\n    -> AddCheese(amount=2)\n    -> Bake(duration=5)\n";
        let recipes = parse_recipes(input).unwrap();
        let r = &recipes["Margherita"];
        let missing = r.missing_actions(&["MakeDough".to_string()]);
        assert!(missing.contains(&"AddCheese".to_string()));
        assert!(missing.contains(&"Bake".to_string()));
    }

    #[test]
    fn test_examples_file() {
        let content = include_str!("../../recipes/examples.recipes");
        let recipes = parse_recipes(content).unwrap();
        assert!(recipes.contains_key("Margherita"));
        assert!(recipes.contains_key("Pepperoni"));
        assert!(recipes.contains_key("QuattroFormaggi"));
    }
}
