module.exports = {
  content: ["./app/**/*.{js,jsx,ts,tsx}", "./components/**/*.{js,jsx,ts,tsx}"],
  theme: {
    extend: {
      colors: {
        'aegis-cyan': '#00d4ff',
        'aegis-magenta': '#ff00ff',
        'aegis-emerald': '#00ff88',
        'aegis-dark': '#0a0a0f',
        'aegis-gray': '#1a1a2e',
        // Noir 2026 landing palette (supports opacity modifiers like /10)
        'primary-neon': 'rgb(168 85 247 / <alpha-value>)',
        'primary-soft': 'rgb(192 132 252 / <alpha-value>)',
        'secondary-neon': 'rgb(96 165 250 / <alpha-value>)',
        'background-base': 'rgb(3 0 20 / <alpha-value>)',
        'surface-dim': 'rgb(255 255 255 / <alpha-value>)',
      },
    },
  },
  plugins: [],
};
