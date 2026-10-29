def L(c):
    r=[]
    for v in c:
        v/=255; r.append(v/12.92 if v<=0.04045 else ((v+0.055)/1.055)**2.4)
    return .2126*r[0]+.7152*r[1]+.0722*r[2]
def cr(a,b):
    a,b=sorted([L(a),L(b)],reverse=True); return (a+.05)/(b+.05)
P={'dark':dict(bg=(10,13,18),card=(17,22,30),fg=(230,237,243),muted=(139,152,168),brand=(45,212,191),brandfg=(4,47,46),ok=(74,222,128),accent=(167,139,250),line=(33,41,54)),
   'light':dict(bg=(250,251,252),card=(255,255,255),fg=(13,17,23),muted=(82,94,110),brand=(15,118,110),brandfg=(255,255,255),ok=(21,128,61),accent=(109,40,217),line=(222,227,233))}
pairs=[('fg','bg',4.5),('fg','card',4.5),('muted','bg',4.5),('muted','card',4.5),('brand','bg',4.5),('brandfg','brand',4.5),('ok','card',4.5),('accent','bg',4.5),('line','bg',None)]
print('| theme | fg / bg | ratio | AA |\n|---|---|---|---|')
for t,p in P.items():
    for f,b,n in pairs:
        r=cr(p[f],p[b]); print(f'| {t} | {f} / {b} | {r:.2f}:1 | {"pass" if n is None or r>=n else "FAIL"}{" (decorative)" if n is None else ""} |')
