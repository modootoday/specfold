// Run from docs/_src: node <tailwindcss>/lib/cli.js -c tailwind.config.js -i input.css -o ../assets/css/site.css --minify
const c=v=>`rgb(var(--${v}) / <alpha-value>)`;
module.exports={darkMode:'class',content:['../_layouts/**/*.html','../_includes/**/*.html','../*.md','../_data/*.yml','../assets/js/*.js','./hero.js'],
theme:{extend:{colors:{bg:c('bg'),fg:c('fg'),muted:c('muted'),line:c('line'),card:c('card'),brand:c('brand'),'brand-fg':c('brand-fg'),ok:c('ok'),accent:c('accent')}}}};
