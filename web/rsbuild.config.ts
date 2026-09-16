import {defineConfig} from '@rsbuild/core';
import {pluginReact} from '@rsbuild/plugin-react';
import tailwind from '@tailwindcss/postcss';
export default defineConfig({
  plugins:[pluginReact()],
  html:{title:'DJI HDMI · Ground station'},
  server:{host:'127.0.0.1',proxy:{'/api':'http://127.0.0.1:8080'}},
  tools:{postcss:{postcssOptions:{plugins:[tailwind()]}}},
});
