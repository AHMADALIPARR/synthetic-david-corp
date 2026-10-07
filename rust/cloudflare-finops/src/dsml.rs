//! Strict DSML subset: one typed read invocation, never code or arbitrary tool names.
use crate::{Result, require};
use serde_json::{Value, json};
const OPEN: &str = "<｜DSML｜tool_calls>";
const INVOKE: &str = "<｜DSML｜invoke name=\"cloudflare_finops_collect\">";
const PARAM: &str = "<｜DSML｜parameter name=\"zone_tag\" string=\"true\">";
const END_PARAM: &str = "</｜DSML｜parameter>";
const END_INVOKE: &str = "</｜DSML｜invoke>";
const CLOSE: &str = "</｜DSML｜tool_calls>";
pub fn example(zone: &str) -> String {
    format!("{OPEN}\n{INVOKE}\n{PARAM}{zone}{END_PARAM}\n{END_INVOKE}\n{CLOSE}")
}
pub fn parse(input: &str, allowed_zone: &str) -> Result<Value> {
    require(input.len() <= 2048, "DSML_TOO_LARGE")?;
    let input = input
        .trim()
        .strip_prefix(OPEN)
        .ok_or("DSML_ENVELOPE_INVALID")?
        .trim();
    let input = input.strip_prefix(INVOKE).ok_or("DSML_TOOL_DENIED")?.trim();
    let input = input.strip_prefix(PARAM).ok_or("DSML_PARAMETER_INVALID")?;
    let (zone, input) = input
        .split_once(END_PARAM)
        .ok_or("DSML_PARAMETER_INVALID")?;
    require(
        zone == allowed_zone && crate::identifier(zone),
        "DSML_ZONE_DENIED",
    )?;
    let input = input
        .trim()
        .strip_prefix(END_INVOKE)
        .ok_or("DSML_PARAMETER_INVALID")?
        .trim();
    require(
        input
            .strip_prefix(CLOSE)
            .is_some_and(|rest| rest.trim().is_empty()),
        "DSML_ENVELOPE_INVALID",
    )?;
    Ok(
        json!({"tool":"cloudflare_finops_collect","arguments":{"zone_tag":zone},"readOnly":true,"requiresLicense":true}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    const ZONE: &str = "189f4aa276a12013fdef48500f6892a2";
    #[test]
    fn exact_envelope_and_forbidden_executable_code() {
        let input = example(ZONE);
        assert_eq!(
            parse(&input, ZONE).unwrap()["tool"],
            "cloudflare_finops_collect"
        );
        assert_eq!(
            parse(
                &input.replace("cloudflare_finops_collect", "tool_UDoH7T4M_execute"),
                ZONE
            )
            .unwrap_err(),
            "DSML_TOOL_DENIED"
        );
        assert!(parse(&input.replace("zone_tag", "code"), ZONE).is_err());
        assert!(parse(&example(&"a".repeat(32)), ZONE).is_err());
        assert!(parse(&(input.clone() + &input), ZONE).is_err());
        assert!(
            parse(
                &input.replace(
                    END_INVOKE,
                    &format!("{PARAM}{}{}{}", ZONE, END_PARAM, END_INVOKE)
                ),
                ZONE
            )
            .is_err()
        );
    }
}
