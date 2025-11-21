import forms from "@tailwindcss/forms";
import daisyui from "daisyui";

/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./main.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      backgroundImage: {
        'gradient-blue': 'linear-gradient(135deg, #003d7a 0%, #0066cc 50%, #0088ff 100%)',
      },
    },
  },
  plugins: [forms, daisyui],
  daisyui: {
    themes: [
      {
        swatto: {
          "primary": "#0066ff",        // Blue
          "primary-content": "#ffffff", // White text on primary
          "secondary": "#004d99",      // Dark blue
          "accent": "#0088ff",         // Light blue accent
          "neutral": "#2a2e37",        // Dark gray for backgrounds
          "base-100": "#1a1d24",       // Dark background
          "base-200": "#24272f",       // Slightly lighter dark
          "base-300": "#2a2e37",       // Card/element background
          "info": "#00aaff",           // Info blue
          "success": "#00d68f",        // Success green
          "warning": "#ffab00",        // Warning amber
          "error": "#ff5630",          // Error red
          
          // CSS variables for gradients
          "--btn-gradient-from": "#003d7a",
          "--btn-gradient-to": "#0088ff",
        },
      },
      "light",
      "dark",
    ],
    darkTheme: "swatto",
  },
};
