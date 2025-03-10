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
		"@typescript-eslint/no-use-before-define": "off",
	},
	env: {
		browser: true,
		es2024: true,
	},
});
