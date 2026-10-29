// Theme toggle, copy buttons, TOC, and lazy renderers for special code fences.
(function(){
var d=document,root=d.documentElement,base=(d.querySelector('link[rel=stylesheet]').getAttribute('href')||'').replace(/css\/site\.css.*$/,'');
var tb=d.getElementById('theme');
function sync(){tb&&tb.setAttribute('aria-pressed',!root.classList.contains('dark'))}
tb&&tb.addEventListener('click',function(){root.classList.toggle('dark');try{localStorage.setItem('theme',root.classList.contains('dark')?'dark':'light')}catch(e){}sync();d.dispatchEvent(new Event('themechange'))});sync();
// Copy buttons with accessible labels and live status.
d.querySelectorAll('pre').forEach(function(p){if(p.closest('.diagram,.language-mermaid,.language-math')||p.querySelector('code.language-mermaid,code.language-math'))return;var b=d.createElement('button');b.type='button';b.className='copy';b.textContent='Copy';b.setAttribute('aria-label','Copy code to clipboard');
 b.onclick=function(){navigator.clipboard.writeText((p.querySelector('code')||p).innerText).then(function(){b.textContent='Copied';setTimeout(function(){b.textContent='Copy'},1500)})};p.appendChild(b)});
d.querySelectorAll('[data-copy-md]').forEach(function(b){b.onclick=function(){fetch(b.dataset.copyMd).then(function(r){return r.text()}).then(function(t){return navigator.clipboard.writeText(t)}).then(function(){b.textContent='Copied'})}});
// Tabs: arrow keys move focus, roving tabindex.
d.querySelectorAll('[data-tabs]').forEach(function(w){var ts=[].slice.call(w.querySelectorAll('[role=tab]'));function sel(t){ts.forEach(function(x){var on=x===t;x.setAttribute('aria-selected',on);x.tabIndex=on?0:-1;d.getElementById(x.getAttribute('aria-controls')).hidden=!on})}
 ts.forEach(function(t,i){t.onclick=function(){sel(t)};t.onkeydown=function(e){var k={ArrowDown:1,ArrowRight:1,ArrowUp:-1,ArrowLeft:-1}[e.key];if(k){e.preventDefault();var n=ts[(i+k+ts.length)%ts.length];sel(n);n.focus()}}})});
// Hero: swap the static SVG for the three.js scene when WebGL works and the device can afford it.
var st=d.getElementById('hero-stage');if(st){var rm=matchMedia('(prefers-reduced-motion: reduce)').matches,low=(navigator.hardwareConcurrency||4)<=2||(navigator.connection&&navigator.connection.saveData),gl=false;try{gl=!!d.createElement('canvas').getContext('webgl2')}catch(e){}
 if(gl&&!low)import(st.dataset.hero).then(function(m){m.mountHero(st,{still:rm});var svg=st.querySelector('svg');svg&&(svg.style.visibility='hidden');st.setAttribute('aria-describedby','hero-desc')}).catch(function(){})}
var toc=d.getElementById('toc');if(toc)d.querySelectorAll('.prose h2[id]').forEach(function(h){var li=d.createElement('li');li.innerHTML='<a class="side" href="#'+h.id+'"></a>';li.firstChild.textContent=h.textContent;toc.appendChild(li)});
function load(src,css){return new Promise(function(ok,no){if(css){var l=d.createElement('link');l.rel='stylesheet';l.href=base+css;d.head.appendChild(l)}var s=d.createElement('script');s.src=base+src;s.onload=ok;s.onerror=no;d.head.appendChild(s)})}
// kramdown emits ```lang fences as .language-<lang> wrappers; find them per language.
function blocks(lang){return Array.prototype.map.call(d.querySelectorAll('.language-'+lang+':not(code), pre > code.language-'+lang),function(el){return el.tagName==='CODE'?el.parentNode:el}).filter(function(v,i,a){return a.indexOf(v)===i})}
function wrap(el,label){var f=d.createElement('figure');f.className='diagram';f.setAttribute('role','group');f.setAttribute('aria-label',label);el.parentNode.insertBefore(f,el);var txt=el.textContent;var src=d.createElement('details');src.innerHTML='<summary>Source</summary>';src.appendChild(el);var out=d.createElement('div');f.appendChild(out);f.appendChild(src);return {fig:f,out:out,text:txt}}
function fail(w,e){w.out.innerHTML='<p class="diagram-error" role="alert"></p>';w.out.firstChild.textContent='Could not render: '+(e&&e.message||e)+'. Showing source.';w.fig.querySelector('details').open=true}
var R={
 mermaid:{js:'vendor/mermaid/mermaid.min.js',run:function(list){var n=0,render=function(){mermaid.initialize({startOnLoad:false,securityLevel:'strict',suppressErrorRendering:true,theme:root.classList.contains('dark')?'dark':'default'});
  list.forEach(function(w){var cap=w.fig.nextElementSibling&&w.fig.nextElementSibling.matches('p')?null:null;mermaid.render('mmd'+(n++),w.text).then(function(r){w.out.innerHTML=r.svg;var s=w.out.querySelector('svg');s.setAttribute('role','img');s.setAttribute('aria-label',w.fig.getAttribute('aria-label'))}).catch(function(e){fail(w,e)})})};render();d.addEventListener('themechange',render)}},
 math:{js:'vendor/katex/katex.min.js',css:'vendor/katex/katex.min.css',run:function(list){list.forEach(function(w){try{katex.render(w.text,w.out,{displayMode:true,output:'htmlAndMathml',throwOnError:true})}catch(e){fail(w,e)}});
  // Inline and display math that kramdown leaves as \( \) and \[ \].
  load('vendor/katex/auto-render.min.js').then(function(){renderMathInElement(d.querySelector('main'),{delimiters:[{left:'\\[',right:'\\]',display:true},{left:'\\(',right:'\\)',display:false}],output:'htmlAndMathml',throwOnError:false})})}}
 // Extension point: graphviz, plantuml, geojson register here with {js, css, run}.
};
Object.keys(R).forEach(function(lang){var els=blocks(lang);var inline=lang==='math'&&/\\\(|\\\[/.test(d.querySelector('main').textContent);if(!els.length&&!inline)return;
 var ws=els.map(function(el){var lab=el.getAttribute('data-label')||(el.previousElementSibling&&el.previousElementSibling.matches('p.diagram-label')?el.previousElementSibling.textContent:lang+' diagram');return wrap(el,lab)});
 load(R[lang].js,R[lang].css).then(function(){R[lang].run(ws)}).catch(function(e){ws.forEach(function(w){fail(w,'renderer failed to load')})})});
})();
