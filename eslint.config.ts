import antfu from "@antfu/eslint-config";

export default antfu({
	formatters: true,
	stylistic: {
		indent: "tab",
		quotes: "double",
		semi: true,
	},
	rules: {
		// 忽略 antfu/top-level-function 规则
		"antfu/top-level-function": "off",
		// no-console 允许info和warn、error
		"no-console": ["error", { allow: ["info", "warn", "error"] }],
		"brace-style": ["error", "1tbs", { allowSingleLine: true }],
	},
});
