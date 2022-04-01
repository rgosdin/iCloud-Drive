# Solanum

## How to build
Run
`npm install` followed by
`npm run build` or `npm run build -- --mode=development`.

Publish the files in the `dist` folder to your webserver.

## How to develop
You can run `npm run serve -- --mode=development` to run a local version
of the frontend.
This will automatically refresh when you change the source code.

Please take a look at `webpack.config.js` and configure the
`devServer.target` to point to a running instance of
[Soil](https://git.sr.ht/~serra/Soil)
