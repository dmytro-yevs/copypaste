/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // JetBrains "New UI" / Darcula-inspired greys.
        ide: {
          bg: "#1e1f22",
          panel: "#2b2d30",
          elevated: "#313438",
          border: "#393b40",
          divider: "#43454a",
          selection: "#2e436e",
          hover: "#34373b",
          text: "#dfe1e5",
          dim: "#9da0a8",
          faint: "#6f737a",
          accent: "#3574f0",
          accentHover: "#4a87f5",
          danger: "#db5c5c",
          success: "#5fad65",
          warning: "#d9a343"
        }
      },
      fontFamily: {
        sans: ["-apple-system", "BlinkMacSystemFont", "Inter", "Segoe UI", "sans-serif"],
        mono: ["ui-monospace", "SFMono-Regular", "JetBrains Mono", "Menlo", "monospace"]
      },
      borderRadius: {
        ide: "6px"
      }
    }
  },
  plugins: []
};
