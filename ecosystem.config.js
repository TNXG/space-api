module.exports = {
	apps: [
		{
			name: "space-api",
			script: "node",
			args: ".output/server/index.mjs",
			cwd: "./",
			instances: 1,
			exec_mode: "fork",
			env: {
				NODE_ENV: "development",
			},
			env_production: {
				NODE_ENV: "production",
			},
			log_date_format: "YYYY-MM-DD HH:mm:ss",
			error_file: "./logs/preview-error.log",
			out_file: "./logs/preview-out.log",
			merge_logs: true,
			max_memory_restart: "1G",
		},
	],
};
