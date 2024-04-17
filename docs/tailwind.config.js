module.exports = {
  purge: ["./templates/**/*.html", "./theme/**/*.html"],
  theme: {
    screens: {
        sm: '640px',
        md: '768px',
        lg: '1024px',
        xl: '1280px',
    },
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