/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        zr: {
          app: "var(--bg-app)",
          surface: "var(--bg-surface)",
          tableHead: "var(--bg-table-head)",
          border: "var(--border-color)",
          primary: "var(--text-primary)",
          secondary: "var(--text-secondary)",
          muted: "var(--text-muted)",
          accent: "var(--color-accent)",
          transfer: "var(--transfer-rate)",
        },
        // Direct Tailwind tokens
        primary: {
          DEFAULT: "var(--text-primary)",
        },
        secondary: {
          DEFAULT: "var(--text-secondary)",
        },
        muted: {
          DEFAULT: "var(--text-muted)",
        },
        accent: {
          DEFAULT: "var(--color-accent)",
        },
      },
      fontSize: {
        '2xs': ['12px', '16px'],
        'xs': ['12px', '16px'],
        'sm': ['13px', '18px'],
        'base': ['14px', '20px'],
      },
    },
  },
  plugins: [],
}
