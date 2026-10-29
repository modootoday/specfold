// Isometric graph: owner, docs, source paths and the check result, drawn with three.js.
import {WebGLRenderer,Scene,OrthographicCamera,BoxGeometry,MeshBasicMaterial,Mesh,EdgesGeometry,LineSegments,LineBasicMaterial,BufferGeometry,Line,Vector3,Group,Color} from 'three';
export function mountHero(host,opts){
  const css=n=>new Color(`rgb(${getComputedStyle(document.documentElement).getPropertyValue('--'+n).trim().split(' ').join(',')})`);
  const r=new WebGLRenderer({antialias:true,alpha:true});r.setPixelRatio(Math.min(devicePixelRatio,2));
  const s=new Scene(),cam=new OrthographicCamera(-6,6,4,-4,0.1,100);cam.position.set(10,10,10);cam.lookAt(0,0,0);
  const g=new Group();s.add(g);const nodes=[];
  const labels=[];const node=(x,z,w,d,h,col,txt)=>{const m=new Mesh(new BoxGeometry(w,h,d),new MeshBasicMaterial({color:css('card')}));m.position.set(x,h/2,z);
    const e=new LineSegments(new EdgesGeometry(m.geometry),new LineBasicMaterial({color:col}));m.add(e);g.add(m);nodes.push(m);if(txt){const el=document.createElement('span');el.textContent=txt;el.className='hero-label';el.setAttribute('aria-hidden','true');host.appendChild(el);labels.push([m,el])}return m};
  const link=(a,b,col)=>{const geo=new BufferGeometry().setFromPoints([a.position.clone().setY(.05),b.position.clone().setY(.05)]);g.add(new Line(geo,new LineBasicMaterial({color:col})))};
  const brand=css('brand'),muted=css('muted'),ok=css('ok');
  const own=node(0,0,1.6,1.6,.9,css('accent'),'owner');
  const d1=node(-3,-1.5,1.8,1.1,.5,brand,'design.md'),d2=node(-3,1.8,1.8,1.1,.5,brand,'plan.md');
  const p=[node(3,-2.4,1.3,.8,.3,muted,'src/cli'),node(3.2,0,1.3,.8,.3,muted,'src/core'),node(3,2.4,1.3,.8,.3,muted,'tests/')];
  const chk=node(0,3.6,2.4,.7,.25,ok,'✓ spec check');
  [d1,d2].forEach(d=>link(d,own,muted));p.forEach(x=>link(own,x,brand));link(own,chk,ok);
  const size=()=>{const w=host.clientWidth,h=host.clientHeight;r.setSize(w,h,false);const a=w/h;cam.left=-4*a;cam.right=4*a;cam.updateProjectionMatrix()};
  host.appendChild(r.domElement);r.domElement.setAttribute('aria-hidden','true');size();addEventListener('resize',size);
  let t=0,mx=0;host.addEventListener('pointermove',e=>{mx=(e.offsetX/host.clientWidth-.5)});
  const tick=()=>{t+=.016;g.rotation.y=opts.still?0:Math.sin(t*.3)*.12+mx*.4;nodes.forEach((n,i)=>{if(!opts.still)n.position.y=n.geometry.parameters.height/2+Math.sin(t*1.5+i)*.06});r.render(s,cam);labels.forEach(([m,el])=>{const v=new Vector3();m.getWorldPosition(v);v.y+=.6;v.project(cam);el.style.left=((v.x+1)/2*100)+'%';el.style.top=((1-v.y)/2*100)+'%'});opts.still||requestAnimationFrame(tick)};
  tick();document.addEventListener('themechange',()=>{s.traverse(o=>{if(o.isMesh)o.material.color=css('card')});r.render(s,cam)});
}
