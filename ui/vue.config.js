module.exports = {
    configureWebpack: {
        node: false,
        externals: {
            'fs-extra': 'commonjs fs-extra',
            'electron': 'commonjs electron',
        },
    },
};
