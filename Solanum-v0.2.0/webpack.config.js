const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const CopyPlugin = require("copy-webpack-plugin");

const config = {
	entry: './src/index.ts',
	mode: 'production',
	module: {
		rules: [
			{
				test: /\.ts$/,
				use: 'ts-loader',
				exclude: /node_modules/,
			}
		],
	},
	resolve: {
		extensions: ['.ts', '.js'],
	},
	plugins: [
		new HtmlWebpackPlugin({
			template: 'src/index.html'
		}),
		new CopyPlugin({
			patterns: [
				{ from: "src/about", to: "" }
			],
		})
	],
	output: {
		filename: 'bundle.js',
		path: path.resolve(__dirname, 'dist'),
		clean: true
	},
	devServer: {
		static: path.join(__dirname, 'dist'),
		proxy: [
			{
				target: 'http://localhost:3030',
				context: ['/libraries', '/directory', '/track', '/inlineTracks'],
				secure: false
			}
		]
	}
}

module.exports = (env, argv) => {
	if (argv.mode === 'development') {
		config.devtool = false;
		config.mode = 'development'
	}
	return config;
};
