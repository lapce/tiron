module.exports = {
  purge: ["./templates/**/*.html", "./theme/**/*.html"],
  theme: {
    fontFamily: {
      body: '"Inter", sans-serif',
      heading: '"Inter", sans-serif',
      sans: '"Inter", sans-serif',
      serif: '"Inter", sans-serif',
      mono: 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
    },
  },
  variants: {},
  plugins: [
      require('@tailwindcss/typography'),
  ],
};