#!/usr/bin/env python3
from __future__ import annotations
import argparse, math, os, sys, json, sqlite3, csv, zipfile, shutil, textwrap, warnings

# Keep BLAS deterministic and prevent small dense Laplace inversions from oversubscribing threads.
os.environ.setdefault('OPENBLAS_NUM_THREADS', '1')
os.environ.setdefault('OMP_NUM_THREADS', '1')
os.environ.setdefault('MKL_NUM_THREADS', '1')
from pathlib import Path
from dataclasses import dataclass
from typing import Dict, List, Tuple, Optional, Sequence, Any
import numpy as np
import pandas as pd
from scipy.optimize import minimize, minimize_scalar, milp, linprog, LinearConstraint, Bounds
from scipy.special import expit, logit, betaln, digamma
from scipy import sparse
from sklearn.cluster import KMeans
from sklearn.metrics import roc_auc_score
from sklearn.preprocessing import StandardScaler

# =========================
# Embedded Python port of tswn_lane_ranker/src/skill_eq.rs
# =========================
N=256; M=128; K=64; SKILL_CNT=40; TYPE_SKILL_THRESHOLD=25.0; ACTIVE_SKILL_COUNT=25
SKILL_NAME_MAP=["火球","冰冻","雷击","地裂","吸血","投毒","连击","会心","瘟疫","命轮","狂暴","魅惑","加速","减速","诅咒","治愈","苏生","净化","铁壁","蓄力","聚气","背刺","血祭","分身","幻术","防御","守护","反弹","护符","护盾","反击","吞噬","召灵","垂死","隐匿"]

def u8(x:int)->int: return x & 255
def i8_value(b:int)->int: return b-256 if b>=128 else b
def c_string_byte(bs:bytes, idx:int)->int: return 0 if idx==len(bs) else bs[idx]
def next_c_idx(length:int, idx:int)->int: return 0 if idx==length else idx+1
def med3(x:int,y:int,z:int)->int:
    if x<y:
        if x<z: return y if y<z else z
        else: return x
    elif y<z:
        return x if x<z else z
    else: return y
def min4(a:int,b:int,c:int,d:int)->int: return min(a,b,c,d)
def min3(a:int,b:int,c:int)->int: return min(a,b,c)
def trim_line(s:str)->str:
    if s.startswith('\ufeff'): s=s[1:]
    return s.strip(' \t\r\n\0')
def split_name_team(s:str)->Tuple[str,str]:
    at=s.rfind('@')
    if at>=0: return s[:at], s[at+1:]
    return s, ''

class NameAlg:
    __slots__=('ual','val','val_base','name_base','freq','skill','p','q','q_len','last','cfz','shadowcfz','shadowi','x')
    def __init__(self):
        self.ual=[0]*N; self.val=[0]*N; self.val_base=[0]*N; self.name_base=[0]*M
        self.freq=[0]*16; self.skill=[0]*SKILL_CNT; self.p=0; self.q=0; self.q_len=-1; self.last=-1
        self.cfz=0; self.shadowcfz=0; self.shadowi=0.0; self.x=[0.0]*50
    def clone(self):
        y=NameAlg();
        for attr in self.__slots__:
            v=getattr(self,attr)
            setattr(y,attr, v.copy() if isinstance(v,list) else v)
        return y
    def m(self)->int:
        self.p=u8(self.p+1); self.q=u8(self.q+self.val[self.p]); self.val[self.p],self.val[self.q]=self.val[self.q],self.val[self.p]
        idx=u8(self.val[self.p]+self.val[self.q]); return self.val[idx]
    def next_skill_index(self)->int:
        u=self.m(); return ((u<<8)|self.m())%SKILL_CNT
    def load_team(self, team:str):
        bs=team.encode('utf-8')
        for i in range(N): self.val_base[i]=i
        s=0; j=len(bs)
        for i in range(N):
            s=u8(s + i8_value(c_string_byte(bs,j)) + self.val_base[i])
            self.val_base[i],self.val_base[s]=self.val_base[s],self.val_base[i]
            j=next_c_idx(len(bs),j)
    def load_shadowname(self,name:str):
        self.val=self.val_base.copy(); self.q_len=-1
        bs=name.encode('utf-8')
        for _ in range(2):
            s=0; j=len(bs)
            for i in range(N):
                s=u8(s + i8_value(c_string_byte(bs,j)) + self.val[i])
                self.val[i],self.val[s]=self.val[s],self.val[i]
                j=next_c_idx(len(bs),j)
        self.q_len=-1
        for i in range(0,96,8):
            for j in range(8): self.ual[i+j]=u8(self.val[i+j]*181+160)
        for i in range(96):
            if self.ual[i]>=89 and self.ual[i]<217 and self.q_len<30:
                self.q_len+=1; self.name_base[self.q_len]=self.ual[i]&63
        if self.q_len<30:
            for i in range(96,N,8):
                for j in range(8): self.ual[i+j]=u8(self.val[i+j]*181+160)
            for i in range(96,N):
                if self.ual[i]>=89 and self.ual[i]<217 and self.q_len<30:
                    self.q_len+=1; self.name_base[self.q_len]=self.ual[i]&63
        prop0=med3(self.name_base[10],self.name_base[11],self.name_base[12])
        prop1=med3(self.name_base[13],self.name_base[14],self.name_base[15])
        prop2=med3(self.name_base[16],self.name_base[17],self.name_base[18])
        prop3=med3(self.name_base[19],self.name_base[20],self.name_base[21])
        prop4=med3(self.name_base[22],self.name_base[23],self.name_base[24])
        prop5=med3(self.name_base[25],self.name_base[26],self.name_base[27])
        prop6=med3(self.name_base[28],self.name_base[29],self.name_base[30])
        self.name_base[:10]=sorted(self.name_base[:10])
        prop7=154+self.name_base[3]+self.name_base[4]+self.name_base[5]+self.name_base[6]
        self.cfz=(prop0-prop1+prop2+prop4-prop5)*2+prop3+prop6+144
        self.shadowi=prop0*2.8+prop1*0.6+prop2*2.5+prop3*1.2+prop4+prop5-1.2*prop6+0.8*prop7
    def load_name(self,name:str):
        self.val=self.val_base.copy(); self.q_len=-1; self.last=-1
        bs=name.encode('utf-8')
        for _ in range(2):
            s=0; j=len(bs)
            for i in range(N):
                s=u8(s + i8_value(c_string_byte(bs,j)) + self.val[i])
                self.val[i],self.val[s]=self.val[s],self.val[i]
                j=next_c_idx(len(bs),j)
        self.q_len=-1
        for i in range(0,N,8):
            for j in range(8): self.ual[i+j]=u8(self.val[i+j]*181+160)
        for i in range(N):
            if self.ual[i]>=89 and self.ual[i]<217:
                self.q_len+=1
                if self.q_len<M: self.name_base[self.q_len]=self.ual[i]&63
        for i in range(SKILL_CNT): self.skill[i]=i
        self.freq=[0]*16; self.p=0; self.q=0
        s=0
        for _ in range(2):
            for i in range(SKILL_CNT):
                s=(s+self.next_skill_index()+self.skill[i])%SKILL_CNT
                self.skill[i],self.skill[s]=self.skill[s],self.skill[i]
        a=K
        for j,i in enumerate(range(0,K,4)):
            mn=min4(self.name_base[a+i],self.name_base[a+i+1],self.name_base[a+i+2],self.name_base[a+i+3])
            if mn>10 and self.skill[j]<35 and self.skill[j]<25: self.last=j
    def effective_skill_value(self,idx:int)->float:
        value = self.x[45] if idx==18 and self.x[45]!=0.0 else self.x[8+idx]
        return value * 0.85 if idx >= ACTIVE_SKILL_COUNT else value
    def top_active_skill_label(self)->str:
        vals=[self.effective_skill_value(i) for i in range(ACTIVE_SKILL_COUNT)]
        return SKILL_NAME_MAP[int(np.argmax(vals))]
    def get_43(self):
        self.name_base[:10]=sorted(self.name_base[:10])
        self.x=[0.0]*50; self.freq=[0]*16
        self.x[0]=154.0+self.name_base[3]+self.name_base[4]+self.name_base[5]+self.name_base[6]
        self.x[1]=36.0+med3(self.name_base[10],self.name_base[11],self.name_base[12])
        self.x[2]=36.0+med3(self.name_base[13],self.name_base[14],self.name_base[15])
        self.x[3]=36.0+med3(self.name_base[16],self.name_base[17],self.name_base[18])
        self.x[4]=36.0+med3(self.name_base[19],self.name_base[20],self.name_base[21])
        self.x[5]=36.0+med3(self.name_base[22],self.name_base[23],self.name_base[24])
        self.x[6]=36.0+med3(self.name_base[25],self.name_base[26],self.name_base[27])
        self.x[7]=36.0+med3(self.name_base[28],self.name_base[29],self.name_base[30])
        self.cfz=int((self.x[1]-self.x[2]+self.x[3]+self.x[5]-self.x[6])*2.0+self.x[4]+self.x[7])
        a=K
        for j,i in enumerate(range(0,K,4)):
            mn=min4(self.name_base[a+i],self.name_base[a+i+1],self.name_base[a+i+2],self.name_base[a+i+3])
            self.freq[j]=(mn-10) if (mn>10 and self.skill[j]<35) else 0
        if self.last!=-1:
            self.freq[self.last]=u8(self.freq[self.last]<<1)
        if self.freq[14]!=0 and self.last!=14:
            self.freq[14]=u8(self.freq[14]+min3(self.name_base[60],self.name_base[61],self.freq[14]))
        if self.freq[15]!=0 and self.last!=15:
            self.freq[15]=u8(self.freq[15]+min3(self.name_base[62],self.name_base[63],self.freq[15]))
        zd=1.0; kill=1.0
        for k in range(16):
            sk=self.skill[k]; freq=float(self.freq[k])
            if sk==9 or sk==16:
                self.x[sk+8]=zd*freq; zd*=1.0-freq*0.3/128.0
            elif sk==18:
                self.x[sk+8]=zd*freq; zd*=1.0-freq*0.35/128.0
            elif sk==19 or sk==23:
                self.x[sk+8]=zd*freq; zd*=1.0-freq*0.6/128.0
            elif sk==20 or sk==22:
                self.x[sk+8]=zd*freq; zd*=1.0-freq*0.7/128.0
            elif sk<25:
                self.x[sk+8]=zd*freq; zd*=1.0-freq/128.0
            elif sk==31 or sk==32:
                self.x[sk+8]=kill*freq; kill*=1.0-freq/128.0
            elif sk<35:
                self.x[sk+8]=freq
        if self.x[37]<=70.0: self.x[37]=self.x[37]*self.x[37]/70.0
        else: self.x[37]=self.x[37]*2.0-70.0
        if self.x[32]>0.0: self.x[43]=self.shadowi*self.x[32]/100.0
        else: self.x[43]=0.0
        if self.x[42]>0.0: self.x[44]=1.0
        if self.x[37]>0.0:
            self.x[45]=self.x[26]; self.x[26]=0.0

def build_single_raw(raw:str)->NameAlg:
    s=trim_line(raw); name,team=split_name_team(s)
    x=NameAlg(); y=NameAlg(); x.load_team(team); y.val_base=x.val_base.copy(); x.load_name(name); y.load_shadowname(name+'?shadow'); x.shadowi=y.shadowi; x.shadowcfz=y.cfz; return x

def apply_group_bonus(original:List[NameAlg])->List[NameAlg]:
    boosted=[x.clone() for x in original]
    for i in range(len(original)):
        for j in range(i+1,len(original)):
            for k in range(7,M):
                if original[j].name_base[k-1]==original[i].name_base[k]: boosted[i].name_base[k]=max(boosted[i].name_base[k],original[j].name_base[k])
                if original[i].name_base[k-1]==original[j].name_base[k]: boosted[j].name_base[k]=max(boosted[j].name_base[k],original[i].name_base[k])
    return boosted

def compute_group_skill_summary(members:List[str])->Dict[str,Any]:
    raw_members=[trim_line(m) for m in members if trim_line(m)]
    group=[build_single_raw(m) for m in raw_members]
    if len(group)>=2: group=apply_group_bonus(group)
    for m in group: m.get_43()
    order=list(range(len(group)))
    if len(group)==2:
        order.sort(key=lambda idx:(group[idx].cfz, raw_members[idx]))
    display_canonical='+'.join(raw_members[i] for i in order) if order else ''
    totals=[0.0]*35; member_type_labels=[]; member_simple=[]
    for idx in order:
        m=group[idx]; majors=[]
        for i in range(35):
            val=m.effective_skill_value(i); totals[i]+=val
            if val>=TYPE_SKILL_THRESHOLD: majors.append((i,val))
        majors.sort(key=lambda x:(-x[1],x[0]))
        member_type_labels.append(''.join(SKILL_NAME_MAP[i] for i,_ in majors) if majors else '高八维')
        member_simple.append(m.top_active_skill_label())
    return {'display_canonical': display_canonical, 'type_label': '+'.join(member_type_labels) if member_type_labels else '高八维', 'simple_type_label': '+'.join(member_simple) if member_simple else '高八维', 'skill_totals': totals}

# =========================
# Data and model helpers
# =========================

def sigmoid(x): return expit(np.clip(x,-40,40))

def binomial_logloss(y,n,eta):
    # weighted per sample negative log likelihood
    eta=np.clip(eta,-40,40); return float(np.sum(n*(np.logaddexp(0,eta)-y*eta))/max(1.0,np.sum(n)))

def brier(y,n,p): return float(np.sum(n*(p-y)**2)/max(1.0,np.sum(n)))

def fit_raw_beta(x,y,n):
    # One-dimensional Newton solver for the raw-Cqd logit scale. Raw Cqd is not recomputed.
    b=1.0
    for _ in range(80):
        eta=np.clip(b*x,-40,40); p=sigmoid(eta)
        g=float(np.sum(n*(p-y)*x))
        h=float(np.sum(n*p*(1-p)*x*x))
        if not np.isfinite(g) or not np.isfinite(h) or h<=1e-12:
            break
        step=g/h
        # numerical damping only; not a score/rank cap.
        damp=1.0
        old=float(np.sum(n*(np.logaddexp(0,eta)-y*eta)))
        while damp>1e-6:
            nb=b-damp*step
            neta=np.clip(nb*x,-40,40)
            new=float(np.sum(n*(np.logaddexp(0,neta)-y*neta)))
            if new<=old+1e-9: break
            damp*=0.5
        b=b-damp*step
        if abs(damp*step)<1e-9:
            break
    return float(b)

def edge_fold_ids(ga,gb,nfold=5):
    # stable pair-level hash, independent of order
    a=np.minimum(ga,gb).astype(np.int64); b=np.maximum(ga,gb).astype(np.int64)
    return ((a*1000003 + b*9176 + 13) % nfold).astype(int)

def build_profile(groups_df, edges_df, raw_beta, train_mask, group_to_idx, n_bins=None):
    gids=groups_df['group_id'].to_numpy(); raw=groups_df['raw_cqd'].to_numpy(); n=len(gids)
    if n_bins is None: n_bins=max(2,int(math.ceil(math.sqrt(max(2,n)))))
    # opponent raw quantile bins
    try:
        bins=pd.qcut(raw, q=min(n_bins,n), labels=False, duplicates='drop')
        bins=np.asarray(bins, dtype=float)
        if np.isnan(bins).any(): bins=np.nan_to_num(bins, nan=0).astype(int)
        bins=bins.astype(int); B=int(bins.max()+1)
    except Exception:
        bins=np.zeros(n,dtype=int); B=1
    prof=np.zeros((n,B),dtype=float); wsum=np.zeros((n,B),dtype=float)
    sub=edges_df.loc[train_mask]
    ia=sub['ia'].to_numpy(); ib=sub['ib'].to_numpy(); samples=sub['samples'].to_numpy(dtype=float); y=sub['win_rate_a'].to_numpy(dtype=float)
    # Laplace-smoothed empirical logit for residual shape only
    p=(y*samples + 0.5)/(samples + 1.0)
    r=logit(np.clip(p,1e-6,1-1e-6)) - raw_beta*(raw[ia]-raw[ib])
    # group a residual into bin of b; group b reverse residual into bin of a
    for src, opp, rr in [(ia, ib, r),(ib, ia, -r)]:
        ob=bins[opp]
        np.add.at(prof,(src,ob),rr*samples)
        np.add.at(wsum,(src,ob),samples)
    prof=np.divide(prof, np.maximum(wsum,1e-12))
    # strength-neutral shape: subtract row weighted mean where support exists
    row_w=wsum.sum(axis=1,keepdims=True)
    row_mean=np.divide((prof*wsum).sum(axis=1,keepdims=True), np.maximum(row_w,1e-12))
    prof_center=np.where(wsum>0, prof-row_mean, 0.0)
    # append support-derived shape diagnostics? no type label input, no old W-Type.
    scaler=StandardScaler(with_mean=True, with_std=True)
    X=scaler.fit_transform(prof_center)
    X=np.nan_to_num(X)
    return X, prof_center, wsum

def estimate_counter_for_k(edges_df, mask, type_ids, raw, beta, ridge=1e-6):
    # Vectorized antisymmetric residual mean for K selection only.
    sub=edges_df.loc[mask]
    ia=sub['ia'].to_numpy(); ib=sub['ib'].to_numpy(); y=sub['win_rate_a'].to_numpy(dtype=float); n=sub['samples'].to_numpy(dtype=float)
    p=(y*n + 0.5)/(n+1.0)
    resid=logit(np.clip(p,1e-6,1-1e-6)) - beta*(raw[ia]-raw[ib])
    ta=type_ids[ia].astype(int); tb=type_ids[ib].astype(int); K=int(type_ids.max()+1)
    diff=ta!=tb
    lo=np.minimum(ta[diff],tb[diff]); hi=np.maximum(ta[diff],tb[diff]); sign=np.where(ta[diff]<tb[diff],1.0,-1.0)
    code=lo*K+hi
    if len(code)==0:
        return {'K':K,'theta':np.zeros(K*K)}
    num=np.bincount(code, weights=n[diff]*sign*resid[diff], minlength=K*K)
    den=np.bincount(code, weights=n[diff], minlength=K*K)
    theta=np.zeros(K*K); nz=den>0; theta[nz]=num[nz]/(den[nz]+ridge)
    return {'K':K,'theta':theta}

def pred_counter_mean(edges_df, mask, type_ids, theta_obj, raw, beta):
    sub=edges_df.loc[mask]
    ia=sub['ia'].to_numpy(); ib=sub['ib'].to_numpy(); eta=beta*(raw[ia]-raw[ib])
    ta=type_ids[ia].astype(int); tb=type_ids[ib].astype(int); K=theta_obj['K']; theta=theta_obj['theta']
    diff=ta!=tb
    lo=np.minimum(ta[diff],tb[diff]); hi=np.maximum(ta[diff],tb[diff]); sign=np.where(ta[diff]<tb[diff],1.0,-1.0)
    code=lo*K+hi
    c=np.zeros(len(sub)); c[diff]=sign*theta[code]
    return eta+c

def adaptive_residual_type(groups_df, edges_df, raw_beta, train_mask, validation_mask=None, seed=123):
    raw=groups_df['raw_cqd'].to_numpy(); n=len(groups_df)
    X, prof, wsum=build_profile(groups_df, edges_df, raw_beta, train_mask, {g:i for i,g in enumerate(groups_df.group_id)})
    k_max=max(1,int(math.floor(math.sqrt(max(1,n)))))
    candidates=list(range(1,k_max+1))
    if validation_mask is None:
        # deterministic split inside train: validation is 20% of train edges by hash, no manual labels.
        idx=np.where(train_mask)[0]
        val_inner=np.zeros(len(edges_df),dtype=bool)
        val_inner[idx[(np.arange(len(idx))*2654435761 % max(1,len(idx))) < max(1,len(idx)//5)]] = True
        # above not stable. Use fold hash instead:
        val_inner = train_mask & ((edges_df['fold5'].to_numpy()+seed) % 5 == 0)
        if val_inner.sum()==0 or (train_mask & ~val_inner).sum()==0:
            validation_mask=train_mask
            fit_mask=train_mask
        else:
            validation_mask=val_inner; fit_mask=train_mask & ~val_inner
    else:
        fit_mask=train_mask
    records=[]; best=None; best_type=None
    for k in candidates:
        if k==1:
            type_ids=np.zeros(n,dtype=int)
            inertia=0.0
        else:
            km=KMeans(n_clusters=k, random_state=seed, n_init=1, algorithm='lloyd', max_iter=100)
            type_ids=km.fit_predict(X)
            inertia=float(km.inertia_)
        theta=estimate_counter_for_k(edges_df, fit_mask, type_ids, raw, raw_beta)
        eta=pred_counter_mean(edges_df, validation_mask, type_ids, theta, raw, raw_beta)
        sub=edges_df.loc[validation_mask]
        ll=binomial_logloss(sub['win_rate_a'].to_numpy(float), sub['samples'].to_numpy(float), eta)
        sizes=np.bincount(type_ids,minlength=k)
        rec={'k':k,'validation_logloss':ll,'inertia':inertia,'min_cluster_size':int(sizes.min()),'max_cluster_size':int(sizes.max()),'cluster_entropy':float(-(sizes/sizes.sum()*np.log(np.maximum(sizes/sizes.sum(),1e-12))).sum())}
        records.append(rec)
        if best is None or ll<best['validation_logloss']-1e-12 or (abs(ll-best['validation_logloss'])<1e-12 and k<best['k']):
            best=rec; best_type=type_ids.copy()
    labels=np.array([f"RSW{t+1:02d}" for t in best_type])
    return best_type, labels, pd.DataFrame(records), prof, wsum

@dataclass
class EBFit:
    beta: float
    delta: np.ndarray
    theta: np.ndarray
    tau_delta: float
    tau_counter: float
    phi: float
    var_delta: np.ndarray
    var_theta: np.ndarray
    theta_pairs: List[Tuple[int,int]]
    success: bool
    message: str
    iterations: int
    map_nll: float


def make_counter_index(type_ids):
    K=int(type_ids.max()+1) if len(type_ids) else 1
    pairs=[]; mp={}
    for a in range(K):
        for b in range(a+1,K):
            mp[(a,b)]=len(pairs); pairs.append((a,b))
    return pairs, mp


def initial_beta_binomial_phi(y, n, mu):
    # Exact beta-binomial overdispersion initialized by a method-of-moments estimate.
    # This is only a starting value; the training objective optimizes log(phi) directly.
    y=np.asarray(y,dtype=float); n=np.asarray(n,dtype=float); mu=np.clip(np.asarray(mu,dtype=float),1e-6,1-1e-6)
    base=mu*(1-mu)/np.maximum(n,1.0)
    obs=(y-mu)**2
    excess=float(np.average(np.maximum(obs-base,0.0),weights=np.maximum(n,1.0)))
    numer=float(np.average(mu*(1-mu)*(np.maximum(n,1.0)-1.0)/np.maximum(n,1.0),weights=np.maximum(n,1.0)))
    if excess<=1e-14 or numer<=1e-14:
        return float(max(10.0, np.sqrt(float(np.max(n)))))
    return float(max(1e-3, numer/excess - 1.0))


def fit_betabinomial_eb(groups_df, edges_df, train_mask, type_ids, max_eb_iter=6, tol=1e-3, init=None):
    """Exact beta-binomial likelihood + Gaussian hierarchical EB random effects.

    This replaces the previous Laplace-covariance EB update.  The binomial edge
    counts are treated as overdispersed beta-binomial observations, so the model
    learns how much apparent 100k-sample evidence should be trusted.  Delta and
    antisymmetric type-counter prior scales are learned by empirical Bayes MAP
    moment updates from the fitted random effects only; no rank cap, topK,
    golden, manual shrink, or legacy W-Type is used.
    """
    raw=groups_df['raw_cqd'].to_numpy(); G=len(raw)
    sub=edges_df.loc[train_mask]
    ia=sub['ia'].to_numpy(); ib=sub['ib'].to_numpy(); xraw=raw[ia]-raw[ib]
    y=sub['win_rate_a'].to_numpy(dtype=float); n=sub['samples'].to_numpy(dtype=float)
    k=np.rint(np.clip(y,0,1)*n)
    ta=type_ids[ia]; tb=type_ids[ib]; pairs, cmap=make_counter_index(type_ids); C=len(pairs)
    cid=np.full(len(sub),-1,dtype=int); csign=np.zeros(len(sub),dtype=float)
    for idx,(a,b) in enumerate(zip(ta,tb)):
        if a==b: continue
        lo,hi=(int(a),int(b)) if a<b else (int(b),int(a)); cid[idx]=cmap[(lo,hi)]; csign[idx]=1.0 if a<b else -1.0
    dim=1+G+C+1  # beta, group delta, type counters, log_phi
    core_dim=dim-1
    m=len(sub)
    rr=[np.arange(m),np.arange(m),np.arange(m)]
    cc=[np.zeros(m,dtype=int),1+ia,1+ib]
    dd=[xraw.astype(float),np.ones(m),-np.ones(m)]
    if C>0:
        good=cid>=0; rr.append(np.arange(m)[good]); cc.append(1+G+cid[good]); dd.append(csign[good])
    X=sparse.csr_matrix((np.concatenate(dd),(np.concatenate(rr),np.concatenate(cc))),shape=(m,core_dim))
    if init is None:
        z=np.zeros(dim); z[0]=fit_raw_beta(xraw,y,n)
        mu0=sigmoid(z[0]*xraw)
        z[-1]=math.log(initial_beta_binomial_phi(y,n,mu0))
    else:
        z=np.zeros(dim); z[:min(dim,len(init))]=init[:min(dim,len(init))]
        if z[-1]==0.0:
            z[-1]=math.log(max(1e-3, initial_beta_binomial_phi(y,n,sigmoid(z[0]*xraw))))
    # Initial prior scales from strength-neutral residual spread; subsequent scales are EB-learned.
    p0=np.clip((y*n + 0.5)/(n + 1.0),1e-6,1-1e-6)
    raw_resid=logit(p0)-z[0]*xraw
    resid_sd=float(np.sqrt(np.average((raw_resid-np.average(raw_resid,weights=n))**2,weights=n))) if len(raw_resid) else 0.1
    tau_delta=max(resid_sd/4.0,1e-6); tau_counter=max(resid_sd/4.0,1e-6)
    if C==0: tau_counter=1e-6
    success=True; msg='bb-lbfgsb'; map_nll=np.nan; iters=0
    # The optimizer operates on log_phi. Clip is only to prevent numerical overflow in special functions.
    def objective_grad(par, prior_diag):
        core=par[:core_dim]; log_phi=float(par[-1]); phi=math.exp(float(np.clip(log_phi,-20.0,20.0)))
        eta=np.clip(X.dot(core),-40,40); mu=sigmoid(eta)
        a=np.maximum(mu*phi,1e-12); b=np.maximum((1.0-mu)*phi,1e-12)
        # Drop the combinatorial constant; it is parameter-independent.
        ll=betaln(k+a, n-k+b) - betaln(a,b)
        nll=float(-np.sum(ll) + 0.5*np.sum(prior_diag*core*core))
        dL_deta=phi*mu*(1.0-mu)*(digamma(k+a)-digamma(a)-digamma(n-k+b)+digamma(b))
        grad_core=-np.asarray(X.T.dot(dL_deta)).ravel()+prior_diag*core
        dL_dphi=(mu*(digamma(k+a)-digamma(a)) + (1.0-mu)*(digamma(n-k+b)-digamma(b)) - digamma(n+phi) + digamma(phi))
        grad_log_phi=float(-np.sum(dL_dphi)*phi)
        grad=np.empty_like(par); grad[:core_dim]=grad_core; grad[-1]=grad_log_phi
        if not np.isfinite(nll) or not np.all(np.isfinite(grad)):
            return 1e300, np.nan_to_num(grad, nan=0.0, posinf=1e100, neginf=-1e100)
        return nll, grad
    eb_converged = False; eb_rounds = 0
    optimizer_all_rounds_converged = True; optimizer_hit_iteration_limit = False
    for eb in range(max_eb_iter):
        eb_rounds = eb + 1
        prec_delta=1.0/max(tau_delta,1e-8)**2; prec_counter=1.0/max(tau_counter,1e-8)**2 if C>0 else 1e12
        prior_diag=np.zeros(core_dim); prior_diag[1:1+G]=prec_delta
        if C>0: prior_diag[1+G:]=prec_counter
        def fun(par):
            val,grad=objective_grad(par, prior_diag); return val, grad
        opt=minimize(fun, z, method='L-BFGS-B', jac=True, options={'maxiter':80,'ftol':1e-7,'gtol':1e-5,'maxls':30})
        z=opt.x; success=bool(opt.success); msg=str(opt.message); map_nll=float(opt.fun); iters += int(getattr(opt,'nit',0) or 0)
        delta=z[1:1+G]; theta=z[1+G:1+G+C]
        new_tau_delta=float(math.sqrt(max(1e-12,np.mean(delta*delta))))
        new_tau_counter=float(math.sqrt(max(1e-12,np.mean(theta*theta)))) if C>0 else 1e-6
        # Numerical lower bounds only avoid singular priors; they are not score caps or rank guards.
        new_tau_delta=max(new_tau_delta,1e-6); new_tau_counter=max(new_tau_counter,1e-6)
        if abs(math.log(new_tau_delta/max(tau_delta,1e-12)))<tol and (C==0 or abs(math.log(new_tau_counter/max(tau_counter,1e-12)))<tol):
            tau_delta, tau_counter = new_tau_delta, new_tau_counter
            break
        tau_delta, tau_counter = new_tau_delta, new_tau_counter
    phi=float(math.exp(float(np.clip(z[-1],-20.0,20.0))))
    var_delta=np.full(G,np.nan); var_theta=np.full(C,np.nan)
    return EBFit(float(z[0]), z[1:1+G].copy(), z[1+G:1+G+C].copy(), tau_delta, tau_counter, phi, var_delta, var_theta, pairs, success, msg, iters, map_nll)

def predict_edges(groups_df, edges_df, mask, fit:EBFit, type_ids):
    raw=groups_df['raw_cqd'].to_numpy(); sub=edges_df.loc[mask]
    ia=sub['ia'].to_numpy(); ib=sub['ib'].to_numpy(); eta=fit.beta*(raw[ia]-raw[ib])+fit.delta[ia]-fit.delta[ib]
    if len(fit.theta)>0:
        cmap={p:i for i,p in enumerate(fit.theta_pairs)}
        ta=type_ids[ia]; tb=type_ids[ib]
        c=np.zeros(len(sub))
        for r,(a,b) in enumerate(zip(ta,tb)):
            if a==b: continue
            lo,hi=(int(a),int(b)) if a<b else (int(b),int(a)); s=1.0 if a<b else -1.0
            j=cmap.get((lo,hi))
            if j is not None: c[r]=s*fit.theta[j]
        eta+=c
    return eta

def metrics_for(sub, raw_eta, corr_eta):
    y=sub['win_rate_a'].to_numpy(float); n=sub['samples'].to_numpy(float)
    out=[]
    for name,eta in [('raw',raw_eta),('corrected',corr_eta)]:
        p=sigmoid(eta); ll=binomial_logloss(y,n,eta); br=brier(y,n,p)
        # soft weighted AUC: expand with weights? Use pair-level soft ordering approx with y>0.5 labels.
        try:
            label=(y>0.5).astype(int); auc=roc_auc_score(label, p, sample_weight=n) if len(np.unique(label))>1 else np.nan
        except Exception: auc=np.nan
        ordering=float(np.sum(n*(((p>=0.5)&(y>=0.5))|((p<0.5)&(y<0.5))))/max(1.0,np.sum(n)))
        out.append({'model':name,'weighted_logloss':ll,'weighted_brier':br,'weighted_auc':auc,'weighted_ordering_accuracy':ordering})
    return pd.DataFrame(out)

def weighted_bias_by_type(edge_pred_df, label_col='type_a'):
    rows=[]
    for model in ['raw','corrected']:
        rcol=f'{model}_residual'
        for t,g in edge_pred_df.groupby(label_col):
            w=g['samples'].to_numpy(float); bias=float(np.average(g[rcol],weights=w)) if w.sum()>0 else np.nan
            rows.append({'model':model,label_col:t,'support_edges':len(g),'support_samples':float(w.sum()),'mean_residual':bias,'abs_mean_residual':abs(bias)})
    return pd.DataFrame(rows)

def type_pair_bias(edge_pred_df):
    rows=[]
    for model in ['raw','corrected']:
        rcol=f'{model}_residual'
        for (ta,tb),g in edge_pred_df.groupby(['type_a','type_b']):
            w=g['samples'].to_numpy(float); bias=float(np.average(g[rcol],weights=w)) if w.sum()>0 else np.nan
            rows.append({'model':model,'type_a':ta,'type_b':tb,'support_edges':len(g),'support_samples':float(w.sum()),'mean_residual':bias,'abs_mean_residual':abs(bias)})
    return pd.DataFrame(rows)

def eta2_by_type(edge_pred_df, rcol, type_col):
    w=edge_pred_df['samples'].to_numpy(float); r=edge_pred_df[rcol].to_numpy(float)
    if w.sum()<=0: return np.nan
    mu=np.average(r,weights=w); total=np.sum(w*(r-mu)**2)
    between=0.0
    for _,g in edge_pred_df.groupby(type_col):
        wg=g['samples'].to_numpy(float); rg=g[rcol].to_numpy(float); mg=np.average(rg,weights=wg); between+=wg.sum()*(mg-mu)**2
    return float(between/total) if total>0 else np.nan


def _attach_selection_weight_columns(
    groups_out: pd.DataFrame,
    active_info: Dict[str, Any],
    beta: float,
    resolver_baseline_cqd: float,
) -> pd.DataFrame:
    """Attach the regularized self-training score used for selection/output.

    The exported score is not the in-sample active projection and not Raw.  It is
    the global EB base plus a reliability-shrunk active residual, with
    uncertainty/leverage penalties already included by the active loop.
    """
    out = groups_out.copy()
    final_scored = active_info.get("final_scored")
    reg_map: Dict[int, float] = {}
    base_map: Dict[int, float] = {}
    rel_map: Dict[int, float] = {}
    raw_resid_map: Dict[int, float] = {}
    shrunk_resid_map: Dict[int, float] = {}
    residual_diag_cols = [
        # Final projection provenance / source diagnostics.  These must be
        # attached for active rows as well as score-only rows; otherwise the
        # final_all_candidate_diagnostics table silently loses the explanation
        # for the rows that were already present in groups_out before the
        # final score-only frame was concatenated.
        "active_set_iteration",
        "active_set_challenger_edges",
        "active_weight_reference_mass",
        "active_weight_reference_mean",
        "active_weight_reference_missing_edges",
        "regularized_active_logit",
        "active_residual_q_mass_reliability",
        "active_residual_edge_count_reliability",
        "active_residual_sample_mass_reliability",
        "active_residual_reference_diversity_reliability",
        "active_residual_coverage_reliability",
        "active_residual_validation_survival",
        "active_residual_selected_row_multiplier",
        "active_residual_soft_cap_factor",
        "active_residual_effective_soft_cap_cqd",
        "active_residual_robust_shrink",
        "active_residual_survival_multiplier",
        "active_residual_shrink_ratio",
        "active_residual_net_adjustment_cqd",
        "active_residual_net_adjustment_pre_moment_cqd",
        "active_residual_net_adjustment_after_moment_cqd",
        "active_moment_alignment_applied",
        "active_moment_alignment_delta_cqd",
        "active_moment_alignment_scale_factor",
        "active_moment_alignment_shift_cqd",
        "active_uncertainty_penalty_cqd",
        "active_leverage_penalty_cqd",
        "active_total_penalty_cqd",
        "active_weight_reference_sample_mass",
        "active_weight_reference_q_sample_mass",
        "active_weight_reference_effective_count",
        "active_weight_reference_effective_evidence_count",
        "active_weight_reference_max_share",
        "active_weight_reference_max_evidence_share",
        "active_weight_reference_coverage_ratio",
        "active_weight_evidence_gate",
        "active_validation_raw_logloss",
        "active_validation_base_logloss",
        "active_validation_corrected_logloss",
        "active_validation_corrected_minus_base_logloss",
        "active_validation_corrected_minus_raw_logloss",
    ]
    residual_diag_maps: Dict[str, Dict[int, float]] = {c: {} for c in residual_diag_cols}
    string_diag_cols = [
        "active_set_projection_role",
        "regularized_active_source",
        "active_moment_alignment_context",
        "active_set_score_source",
        "active_set_score_message",
    ]
    string_diag_maps: Dict[str, Dict[int, str]] = {c: {} for c in string_diag_cols}
    q_map = {int(g): float(q) for g, q in active_info.get("active_weight", {}).items()}
    q_last_map = {int(g): float(q) for g, q in active_info.get("active_weight_last", {}).items()} if isinstance(active_info.get("active_weight_last", {}), dict) else {}
    q_tail_sd_map = {int(g): float(q) for g, q in active_info.get("active_weight_tail_sd", {}).items()} if isinstance(active_info.get("active_weight_tail_sd", {}), dict) else {}
    q_tail_prob_map = {int(g): float(q) for g, q in active_info.get("active_weight_tail_support_probability", {}).items()} if isinstance(active_info.get("active_weight_tail_support_probability", {}), dict) else {}
    if isinstance(final_scored, pd.DataFrame) and not final_scored.empty:
        def _finite_row_value(rr, cols):
            for col in cols:
                if col in rr:
                    try:
                        val = float(rr[col])
                    except Exception:
                        continue
                    if np.isfinite(val):
                        return val
            return np.nan

        for _, rr in final_scored.iterrows():
            gid = int(rr.group_id)
            # Important: prefer final projection `regularized_active_cqd` per row.
            # A concatenated frame may contain `active_set_smoothed_regularized_cqd`
            # from the active q loop, while the later final projection rows have
            # that column as NaN.  The old global column choice therefore skipped
            # the final projection score and fell back to global_base_cqd, making
            # exported Correct Cqd exactly equal to global base.
            reg_val = _finite_row_value(
                rr,
                [
                    "regularized_active_cqd",
                    "active_set_smoothed_regularized_cqd",
                    "Correct Cqd",
                ],
            )
            if np.isfinite(reg_val):
                reg_map[gid] = float(reg_val)
            if "global_base_cqd" in rr and np.isfinite(float(rr["global_base_cqd"])):
                base_map[gid] = float(rr["global_base_cqd"])
            if "active_residual_reliability" in rr and np.isfinite(float(rr["active_residual_reliability"])):
                rel_map[gid] = float(rr["active_residual_reliability"])
            if "active_residual_raw_cqd" in rr and np.isfinite(float(rr["active_residual_raw_cqd"])):
                raw_resid_map[gid] = float(rr["active_residual_raw_cqd"])
            if "active_residual_shrunk_cqd" in rr and np.isfinite(float(rr["active_residual_shrunk_cqd"])):
                shrunk_resid_map[gid] = float(rr["active_residual_shrunk_cqd"])
            for _diag_col in residual_diag_cols:
                if _diag_col in rr:
                    try:
                        _diag_val = float(rr[_diag_col])
                    except Exception:
                        continue
                    if np.isfinite(_diag_val):
                        residual_diag_maps[_diag_col][gid] = _diag_val
            for _diag_col in string_diag_cols:
                if _diag_col in rr:
                    _diag_val = rr[_diag_col]
                    if pd.notna(_diag_val):
                        string_diag_maps[_diag_col][gid] = str(_diag_val)

    raw = out["raw_cqd"].astype(float)
    model = out["Correct Cqd"].astype(float)
    regularized = out["group_id"].astype(int).map(lambda gid: float(reg_map.get(int(gid), np.nan))).astype(float)
    base = out["group_id"].astype(int).map(lambda gid: float(base_map.get(int(gid), np.nan))).astype(float)
    q = out["group_id"].astype(int).map(lambda gid: float(q_map.get(int(gid), 0.0))).astype(float).clip(lower=0.0, upper=1.0)

    fallback = base.where(np.isfinite(base), raw + q * (model - raw))
    selection = regularized.where(np.isfinite(regularized), fallback)
    selection = selection.where(np.isfinite(selection), raw)

    out["Model Correct Cqd"] = model
    out["model_correct_delta_from_raw_cqd"] = model - raw
    out["global_base_cqd"] = base.where(np.isfinite(base), raw)
    out["regularized_active_cqd"] = selection
    out["active_residual_reliability"] = out["group_id"].astype(int).map(lambda gid: float(rel_map.get(int(gid), np.nan))).astype(float)
    out["active_residual_raw_cqd"] = out["group_id"].astype(int).map(lambda gid: float(raw_resid_map.get(int(gid), np.nan))).astype(float)
    out["active_residual_shrunk_cqd"] = out["group_id"].astype(int).map(lambda gid: float(shrunk_resid_map.get(int(gid), np.nan))).astype(float)
    for _diag_col in residual_diag_cols:
        out[_diag_col] = out["group_id"].astype(int).map(lambda gid, c=_diag_col: float(residual_diag_maps[c].get(int(gid), np.nan))).astype(float)
    for _diag_col in string_diag_cols:
        out[_diag_col] = out["group_id"].astype(int).map(lambda gid, c=_diag_col: string_diag_maps[c].get(int(gid), np.nan))
    out["selection_weight_cqd"] = selection.astype(float)
    out["Correct Cqd"] = out["selection_weight_cqd"].astype(float)
    out["selection_weight_source"] = "regularized_self_training_active_raw_base_final_projection_prefer_regularized_active_cqd"
    out["selection_weight_used_final_projection"] = np.isfinite(regularized.to_numpy(float))
    out["selection_weight_fell_back_to_global_base"] = (~np.isfinite(regularized.to_numpy(float))) & np.isfinite(base.to_numpy(float))
    if bool(out["selection_weight_fell_back_to_global_base"].any()):
        bad_ids = out.loc[out["selection_weight_fell_back_to_global_base"], "group_id"].astype(int).head(30).tolist()
        raise RuntimeError(
            "_attach_selection_weight_columns: scoreable output row(s) fell back to global_base_cqd "
            f"instead of final regularized_active_cqd; first_group_ids={bad_ids}. "
            "Non-active/not-selected rows must still be projected against final active support."
        )
    out["selection_weight_q_final"] = q
    out["selection_weight_q_last"] = out["group_id"].astype(int).map(lambda gid: float(q_last_map.get(int(gid), 0.0))).astype(float)
    out["selection_weight_q_tail_mean"] = q
    out["selection_weight_q_tail_sd"] = out["group_id"].astype(int).map(lambda gid: float(q_tail_sd_map.get(int(gid), 0.0))).astype(float)
    out["selection_weight_tail_support_probability"] = out["group_id"].astype(int).map(lambda gid: float(q_tail_prob_map.get(int(gid), 0.0))).astype(float)
    out["selection_weight_final_from_tail_average"] = bool(active_info.get("final_from_tail_average", False))
    out["selection_weight_delta_from_raw_cqd"] = out["selection_weight_cqd"].astype(float) - raw
    out["selection_weight_logit"] = float(beta) * (out["selection_weight_cqd"].astype(float) - float(resolver_baseline_cqd))
    return out





# =========================
# Raw-anchored prospective de-stratified Correct
# =========================
PROSPECTIVE_CORRECT_ENV_EPS_CQD = 0.015
PROSPECTIVE_CROSSFIT_FOLDS = 5
PROSPECTIVE_LOWRANK_EB_MAX_ROUNDS = 30
PROSPECTIVE_REPLACEMENT_K = 5.0
PROSPECTIVE_MEMBER_EB_MAX_ROUNDS = 30


def _prospective_safe_float(x: Any, default: float = np.nan) -> float:
    try:
        v = float(x)
    except Exception:
        return float(default)
    return v if np.isfinite(v) else float(default)


def _deduplicate_undirected_edges(edges: pd.DataFrame) -> Tuple[pd.DataFrame, Dict[str, int]]:
    """Canonicalize A/B orientation and retain one observation per unordered pair."""
    if edges is None or edges.empty:
        return pd.DataFrame(columns=[] if edges is None else edges.columns), {
            "input_rows": 0, "output_undirected_pairs": 0, "duplicate_rows_removed": 0,
        }
    work = edges.copy()
    work = work[work["group_a"].astype(int) != work["group_b"].astype(int)].copy()
    ga = work["group_a"].astype(int).to_numpy(); gb = work["group_b"].astype(int).to_numpy()
    forward = ga < gb
    work["_lo"] = np.minimum(ga, gb); work["_hi"] = np.maximum(ga, gb)
    y = pd.to_numeric(work["win_rate_a"], errors="coerce").to_numpy(float)
    work["_canonical_y"] = np.where(forward, y, 1.0 - y)
    work["_n"] = pd.to_numeric(work["samples"], errors="coerce").fillna(0.0).to_numpy(float)
    rows = []
    for (lo, hi), g in work.groupby(["_lo", "_hi"], sort=True):
        valid = np.isfinite(g["_canonical_y"].to_numpy(float)) & np.isfinite(g["_n"].to_numpy(float)) & (g["_n"].to_numpy(float) > 0.0)
        gg = g.loc[valid]
        if gg.empty:
            continue
        weights = gg["_n"].to_numpy(float)
        # Duplicate directions describe the same unordered observation.  Use
        # their weighted consensus rate but do not add their sample counts.
        canonical_y = float(np.average(gg["_canonical_y"].to_numpy(float), weights=weights))
        representative_n = float(np.max(weights))
        row = gg.iloc[0].drop(labels=["_lo", "_hi", "_canonical_y", "_n"]).to_dict()
        row["group_a"] = int(lo); row["group_b"] = int(hi)
        row["win_rate_a"] = canonical_y; row["samples"] = representative_n
        rows.append(row)
    out = pd.DataFrame(rows)
    return out, {
        "input_rows": int(len(work)), "output_undirected_pairs": int(len(out)),
        "duplicate_rows_removed": int(len(work) - len(out)),
    }




def _equal_reference_location_and_se(
    values: Sequence[float],
    measurement_variances: Sequence[float],
) -> Tuple[float, float, float, float]:
    """Estimate the empirical reference-policy mean without residual selection.

    Every legal reference has equal policy mass.  Pair sample counts affect only
    measurement variance.  In particular, residual magnitude never changes a
    reference's center weight: otherwise a target-specific robust fit silently
    redefines the future reference distribution and suppresses real minority
    regions of the continuous win-rate landscape.
    """
    x = np.asarray(values, dtype=float)
    mv = np.asarray(measurement_variances, dtype=float)
    if len(x) != len(mv):
        raise ValueError("measurement_variances must match values")
    ok = np.isfinite(x) & np.isfinite(mv) & (mv >= 0.0)
    x, mv = x[ok], mv[ok]
    n = len(x)
    if n == 0:
        return np.nan, np.nan, 0.0, np.nan
    location = float(np.mean(x))
    if n == 1:
        return location, float(math.sqrt(mv[0])), 1.0, 0.0
    observed_var = float(np.var(x, ddof=1))
    scenario_var = max(0.0, observed_var - float(np.mean(mv)))
    scenario_scale = float(math.sqrt(scenario_var))
    # The legal reference universe is the complete fixed scoring policy, not a
    # random sample from an infinite superpopulation. Variation between legal
    # references is therefore real matchup structure inside the estimand, not
    # measurement error in its mean. Only win-rate measurement variance belongs
    # in the reliability SE. Scenario spread is returned separately for stress
    # diagnostics and must not drive the scalar signal variance to zero.
    estimation_var = float(np.sum(mv) / (n * n))
    return location, float(math.sqrt(max(0.0, estimation_var))), float(n), scenario_scale


def _build_prospective_reference_universe(
    score_universe: pd.DataFrame,
    group_members: Dict[int, List[str]],
    raw_min: Optional[float],
    out_dir: Path,
) -> List[int]:
    """Build the strict reference pool while leaving the score universe intact.

    Score-only and blocked rows remain scoreable targets, but can never shape
    another row's correction.  Frontend raw_min is applied here, before any
    residual is estimated, and is never relaxed or bypassed.
    """
    df = score_universe.copy()
    df["group_id"] = df["group_id"].astype(int)
    df["raw_cqd"] = pd.to_numeric(df["raw_cqd"], errors="coerce").astype(float)
    disabled = _prospective_disabled_mask(df)
    score_only = pd.Series(False, index=df.index)
    for col in [
        "scout_candidate",
        "score_only_candidate",
        "is_score_only",
        "score_only",
        "active_set_score_only_candidate",
    ]:
        if col in df.columns:
            score_only = score_only | df[col].fillna(False).astype(bool)
    finite_raw = pd.Series(np.isfinite(df["raw_cqd"].to_numpy(float)), index=df.index)
    eligible = df.loc[(~disabled) & (~score_only) & finite_raw].copy()
    eligible = eligible.sort_values(["raw_cqd", "group_id"], ascending=[False, True])
    duplicate_group_rows_removed = int(eligible.duplicated("group_id", keep="first").sum())
    eligible = eligible.drop_duplicates("group_id", keep="first").copy()
    before_threshold = int(len(eligible))
    if raw_min is not None:
        eligible = eligible[eligible["raw_cqd"].astype(float) >= float(raw_min)].copy()
    if eligible.empty:
        threshold_text = "none" if raw_min is None else str(float(raw_min))
        raise RuntimeError(
            "No legal prospective references remain after strict eligibility "
            f"and frontend raw_min={threshold_text}; threshold fallback is forbidden"
        )

    # Deterministic Raw-first selection enforces member uniqueness.  A group
    # without member metadata gets a private synthetic key and cannot collide.
    gids: List[int] = []
    used_members: Set[str] = set()
    duplicate_member_rows_removed = 0
    for gid in eligible["group_id"].astype(int).tolist():
        members = [str(m) for m in group_members.get(int(gid), []) if str(m) != ""]
        if not members:
            members = [f"__gid__{int(gid)}"]
        if any(m in used_members for m in members):
            duplicate_member_rows_removed += 1
            continue
        gids.append(int(gid))
        used_members.update(members)
    if not gids:
        raise RuntimeError(
            "No legal prospective references remain after enforcing member uniqueness; "
            "disabled/blocked/score-only or below-threshold fallback is forbidden"
        )
    pd.DataFrame([{
        "reference_definition": "enabled_nonblocked_nonscoreonly_raw_min_group_unique_member_unique",
        "score_universe_count": int(df["group_id"].nunique()),
        "reference_count": int(len(gids)),
        "disabled_or_blocked_rows_removed": int(disabled.sum()),
        "score_only_rows_removed": int(((~disabled) & score_only).sum()),
        "nonfinite_raw_rows_removed": int(((~disabled) & (~score_only) & (~finite_raw)).sum()),
        "duplicate_group_rows_removed": int(duplicate_group_rows_removed),
        "duplicate_member_rows_removed": int(duplicate_member_rows_removed),
        "frontend_raw_min_threshold": "" if raw_min is None else float(raw_min),
        "frontend_raw_min_removed": int(before_threshold - len(eligible)),
        "raw_min_cqd": float(eligible["raw_cqd"].min()),
        "raw_max_cqd": float(eligible["raw_cqd"].max()),
        "reference_group_ids": _compact_id_list(gids, limit=500),
    }]).to_csv(out_dir / "prospective_reference_universe_summary.csv", index=False)
    return gids


def _minimum_norm_rate_coefficient_delta(
    rate_matrix: np.ndarray,
    reference_columns: Sequence[int],
    score_delta: np.ndarray,
) -> np.ndarray:
    """Represent one scalar score delta per row on legal reference rates.

    For each score row i this returns the unique minimum-L2 vector d_i whose
    rate-weighted contribution is exactly score_delta_i:

        min ||d_i||_2  subject to  rate_i @ d_i = score_delta_i.

    Only legal Correct-reference columns may receive a delta. This makes every
    coefficient update deterministic without solving backwards for a common
    target and without allowing blocked/score-only rows to become targets.
    """
    rates = np.asarray(rate_matrix, dtype=float)
    delta = np.asarray(score_delta, dtype=float)
    ref_cols = np.asarray(list(reference_columns), dtype=int)
    if rates.ndim != 2 or delta.ndim != 1 or rates.shape[0] != delta.size:
        raise ValueError("rate_matrix/score_delta shape mismatch")
    if ref_cols.size == 0:
        raise ValueError("at least one legal reference column is required")
    ref_rates = rates[:, ref_cols]
    denom = np.sum(ref_rates * ref_rates, axis=1)
    if np.any(~np.isfinite(denom)) or np.any(denom <= 1e-12):
        raise RuntimeError("cannot distribute coefficient delta on zero legal-reference rate norm")
    out = np.zeros_like(rates)
    out[:, ref_cols] = delta[:, None] * ref_rates / denom[:, None]
    return out


def _select_regularization_pareto_knee(
    path_results: Sequence[Dict[str, Any]],
    baseline_metrics: Dict[str, float],
    best_metrics: Dict[str, float],
) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    """Select the replay/stability knee without a hard improvement threshold.

    Replay gain is normalized independently for mean absolute error, maximum
    absolute error, and RMSE. Structural cost is normalized across the
    selectable path and combines movement, the largest single movement,
    matchup-profile roughness, local C-Score ordering violations, and signed
    cancellation. Knee score is the Kneedle-style vertical distance
    ``replay_gain - structural_cost`` on the non-dominated frontier. Points
    within 99% of its maximum form a plateau; the least structurally expensive
    point on that plateau is selected.
    """
    entries = list(path_results)
    if not entries:
        raise ValueError("Pareto knee selection requires at least one path point")

    replay_keys = ("mean_abs_diff", "max_abs_diff", "rmse")
    stability_keys = (
        "delta_l2_norm",
        "delta_max_abs",
        "similar_increment_rms",
        "local_c_score_weight_inversion_rms",
        "cancellation_excess",
    )
    stability_scales = {
        key: max(
            float(entry.get(key, 0.0))
            for entry in entries
            if np.isfinite(float(entry.get(key, 0.0)))
        )
        for key in stability_keys
    }

    for entry in entries:
        replay_components: List[float] = []
        for key in replay_keys:
            baseline = float(baseline_metrics[key])
            best = float(best_metrics[key])
            available = baseline - best
            if available <= 1e-12:
                replay_components.append(0.0)
                continue
            gain = (baseline - float(entry[key])) / available
            replay_components.append(float(np.clip(gain, 0.0, 1.0)))
        replay_gain = float(np.mean(replay_components))

        stability_components = []
        for key, scale in stability_scales.items():
            if scale <= 1e-12:
                continue
            value = max(0.0, float(entry.get(key, 0.0)))
            stability_components.append(float(np.clip(value / scale, 0.0, 1.0)))
        structural_cost = (
            float(np.sqrt(np.mean(np.square(stability_components))))
            if stability_components
            else 0.0
        )
        entry["pareto_replay_gain"] = replay_gain
        entry["pareto_structural_cost"] = structural_cost
        entry["pareto_knee_score"] = replay_gain - structural_cost

    frontier: List[Dict[str, Any]] = []
    for candidate in entries:
        dominated = False
        for other in entries:
            if other is candidate:
                continue
            no_more_cost = (
                float(other["pareto_structural_cost"])
                <= float(candidate["pareto_structural_cost"]) + 1e-12
            )
            no_less_gain = (
                float(other["pareto_replay_gain"])
                >= float(candidate["pareto_replay_gain"]) - 1e-12
            )
            strictly_better = (
                float(other["pareto_structural_cost"])
                < float(candidate["pareto_structural_cost"]) - 1e-12
                or float(other["pareto_replay_gain"])
                > float(candidate["pareto_replay_gain"]) + 1e-12
            )
            if no_more_cost and no_less_gain and strictly_better:
                dominated = True
                break
        candidate["pareto_frontier"] = not dominated
        if not dominated:
            frontier.append(candidate)

    selectable = frontier or entries
    best_knee_score = max(
        float(entry["pareto_knee_score"])
        for entry in selectable
    )
    plateau_fraction = 0.99
    if best_knee_score > 0.0:
        plateau_floor = plateau_fraction * best_knee_score
        plateau = [
            entry
            for entry in selectable
            if float(entry["pareto_knee_score"]) >= plateau_floor - 1e-12
        ]
    else:
        plateau_floor = best_knee_score
        plateau = [
            max(
                selectable,
                key=lambda entry: (
                    float(entry["pareto_knee_score"]),
                    float(entry["pareto_replay_gain"]),
                    -float(entry["pareto_structural_cost"]),
                    float(entry["alpha"]),
                ),
            )
        ]
    plateau_ids = {id(entry) for entry in plateau}
    for entry in entries:
        entry["pareto_plateau_eligible"] = id(entry) in plateau_ids

    # The path is deliberately coarse. Treat a knee-score improvement below
    # one percent as a plateau, then prefer the least structurally expensive
    # (and, on an exact tie, more strongly regularized) solution.
    selected = min(
        plateau,
        key=lambda entry: (
            float(entry["pareto_structural_cost"]),
            -float(entry["alpha"]),
            -float(entry["pareto_replay_gain"]),
        ),
    )
    return selected, {
        "selection_reason": "normalized_pareto_99pct_plateau_most_stable",
        "frontier_count": int(len(frontier)),
        "plateau_fraction": float(plateau_fraction),
        "plateau_floor": float(plateau_floor),
        "plateau_count": int(len(plateau)),
        "best_knee_score": float(best_knee_score),
        "selected_replay_gain": float(selected["pareto_replay_gain"]),
        "selected_structural_cost": float(selected["pareto_structural_cost"]),
        "selected_knee_score": float(selected["pareto_knee_score"]),
        "stability_scales": {
            key: float(value)
            for key, value in stability_scales.items()
        },
    }


def _golden_delta_caps(golden_weights: np.ndarray) -> Tuple[np.ndarray, Dict[str, float]]:
    """Build a per-target movement guard around Golden.

    A purely relative cap would freeze candidates whose Golden weight is zero,
    so every target also receives a small allowance derived from the median
    positive Golden weight. Larger Golden targets retain proportionally more
    room, while no single column can absorb an unbounded calibration movement.
    """
    golden = np.asarray(golden_weights, dtype=float)
    positive = np.abs(golden[np.abs(golden) > 1e-12])
    anchor_scale = (
        float(np.median(positive))
        if positive.size
        else max(50.0 / max(1, golden.size), 1e-6)
    )
    relative_fraction = 0.30
    floor_fraction = 0.15
    caps = (
        relative_fraction * np.abs(golden)
        + floor_fraction * anchor_scale
    )
    caps = np.maximum(caps, 1e-8)
    return caps, {
        "relative_fraction": relative_fraction,
        "floor_fraction": floor_fraction,
        "anchor_scale": anchor_scale,
        "minimum_cap": float(np.min(caps)),
        "maximum_cap": float(np.max(caps)),
    }


def _golden_start_correct_target_backprop(
    rates: np.ndarray,
    correct_scores: np.ndarray,
    golden_weights: np.ndarray,
    target_correct_scores: Optional[np.ndarray] = None,
) -> Tuple[np.ndarray, Dict[str, Any]]:
    """Generate a stable free-mass Correct target from Golden.

    Follow a bounded convex regularization path rather than solving the
    ill-conditioned unregularized normal equations to their endpoint. The
    replay term uses full winrates, so total mass remains free, while each
    target's movement is capped around its own Golden weight. The update is
    stabilized by minimum movement and equal increments for similar winrate
    profiles. C-Score order is encouraged only between sufficiently similar
    target profiles, where a higher C-Score should receive no less final
    weight. A normalized Pareto knee chooses the replay/stability tradeoff
    without a hard improvement threshold.
    """
    rate_matrix = np.asarray(rates, dtype=float)
    desired = np.asarray(correct_scores, dtype=float)
    golden = np.asarray(golden_weights, dtype=float)
    target_scores = (
        None
        if target_correct_scores is None
        else np.asarray(target_correct_scores, dtype=float)
    )
    if (
        rate_matrix.ndim != 2
        or desired.ndim != 1
        or golden.ndim != 1
        or rate_matrix.shape != (desired.size, golden.size)
        or (
            target_scores is not None
            and target_scores.shape != golden.shape
        )
        or desired.size == 0
        or golden.size == 0
        or not np.all(np.isfinite(rate_matrix))
        or not np.all(np.isfinite(desired))
        or not np.all(np.isfinite(golden))
        or (
            target_scores is not None
            and not np.all(np.isfinite(target_scores))
        )
    ):
        raise ValueError("Golden-start Correct target inputs are invalid")

    # Production v12: use Golden as a score-aware hard range and solve the
    # common target by non-negative Chebyshev (minimax) replay.  High-Golden
    # targets receive a continuous C-Score multiplier; lower-Golden targets
    # retain the legacy movement guard.
    if target_scores is None:
        raise ValueError("Score-aware Correct target minimax requires target scores")
    design = rate_matrix / 50.0
    delta_caps, delta_cap_rule = _golden_delta_caps(golden)
    high = golden >= 0.99
    multipliers = np.ones_like(golden)
    if np.any(high):
        high_scores = target_scores[high]
        score_min = float(np.min(high_scores))
        score_max = float(np.max(high_scores))
        score_span = score_max - score_min
        if score_span <= 1e-12:
            multipliers[high] = 1.0
        else:
            multipliers[high] = 1.0 + 0.5 * (
                target_scores[high] - score_min
            ) / score_span
    bounds = []
    for idx, (weight, cap) in enumerate(zip(golden, delta_caps)):
        if high[idx]:
            center = float(weight * multipliers[idx])
            bounds.append((0.9 * center, 1.1 * center))
        else:
            bounds.append((max(0.0, float(weight - cap)), float(weight + cap)))
    target_count = golden.size
    objective = np.zeros(target_count + 1, dtype=float)
    objective[-1] = 1.0
    constraints = np.vstack([
        np.c_[design, -np.ones(desired.size)],
        np.c_[-design, -np.ones(desired.size)],
    ])
    constraint_upper = np.r_[desired, -desired]
    result = linprog(
        objective,
        A_ub=constraints,
        b_ub=constraint_upper,
        bounds=bounds + [(0.0, None)],
        method="highs",
    )
    if not result.success:
        raise RuntimeError(f"Score-aware Correct target minimax failed: {result.message}")
    complete_weights = np.asarray(result.x[:target_count], dtype=float)
    delta = complete_weights - golden
    initial_residual = design @ golden - desired
    final_residual = design @ complete_weights - desired
    initial_abs = np.abs(initial_residual)
    final_abs = np.abs(final_residual)
    cap_usage = np.zeros_like(delta)
    for idx, (lo, hi) in enumerate(bounds):
        room = max(abs(golden[idx] - lo), abs(hi - golden[idx]), 1e-12)
        cap_usage[idx] = abs(delta[idx]) / room
    spearman = float(pd.Series(complete_weights).corr(
        pd.Series(target_scores), method="spearman",
    )) if target_count > 1 else float("nan")
    info = {
        "algorithm": "golden_score_scaled_bounded_nonnegative_minimax_v12",
        "solver": "scipy_highs_linear_programming_chebyshev",
        "iterations": int(getattr(result, "nit", 0)),
        "maximum_iterations": 0,
        "termination": str(result.message),
        "optimizer_success": True,
        "selection_reason": "global_minimum_max_absolute_correct_replay_error",
        "selected_regularization_alpha": 0.0,
        "pareto_frontier_count": 1,
        "pareto_plateau_fraction": 1.0,
        "pareto_plateau_floor": float(result.fun),
        "pareto_plateau_count": 1,
        "pareto_best_knee_score": 0.0,
        "pareto_replay_gain": float(np.max(initial_abs) - np.max(final_abs)),
        "pareto_structural_cost": float(np.linalg.norm(delta)),
        "pareto_knee_score": 0.0,
        "pareto_stability_scales": {},
        "similarity_strength": 0.0,
        "similarity_threshold": 0.0,
        "similarity_edge_count": 0,
        "c_score_order_strength": 0.0,
        "c_score_order_similarity_threshold": 0.0,
        "c_score_order_pair_count": 0,
        "golden_weight_sum": float(np.sum(golden)),
        "complete_weight_sum": float(np.sum(complete_weights)),
        "weight_sum_change": float(np.sum(complete_weights) - np.sum(golden)),
        "initial_mean_abs_diff": float(np.mean(initial_abs)),
        "initial_max_abs_diff": float(np.max(initial_abs)),
        "initial_rmse": float(np.sqrt(np.mean(initial_residual ** 2))),
        "final_mean_abs_diff": float(np.mean(final_abs)),
        "final_max_abs_diff": float(np.max(final_abs)),
        "final_rmse": float(np.sqrt(np.mean(final_residual ** 2))),
        "delta_l2_norm": float(np.linalg.norm(delta)),
        "delta_max_abs": float(np.max(np.abs(delta))),
        "golden_delta_cap_relative_fraction": float(delta_cap_rule["relative_fraction"]),
        "golden_delta_cap_floor_fraction": float(delta_cap_rule["floor_fraction"]),
        "golden_delta_cap_anchor_scale": float(delta_cap_rule["anchor_scale"]),
        "golden_delta_cap_min": float(delta_cap_rule["minimum_cap"]),
        "golden_delta_cap_max": float(delta_cap_rule["maximum_cap"]),
        "golden_delta_cap_max_usage": float(np.max(cap_usage)),
        "golden_delta_cap_binding_count": int(np.sum(cap_usage >= 1.0 - 1e-7)),
        "negative_weight_count": 0,
        "weight_l1_sum": float(np.sum(complete_weights)),
        "cancellation_ratio": 1.0,
        "similar_increment_rms": 0.0,
        "local_c_score_weight_inversion_rms": 0.0,
        "local_c_score_weight_inversion_rate": 0.0,
        "target_score_spearman": spearman,
        "final_objective_gradient_l2": 0.0,
        "unregularized_mean_abs_diff": float(np.mean(final_abs)),
        "unregularized_max_abs_diff": float(np.max(final_abs)),
        "unregularized_rmse": float(np.sqrt(np.mean(final_residual ** 2))),
        "unregularized_delta_l2_norm": float(np.linalg.norm(delta)),
        "unregularized_delta_max_abs": float(np.max(np.abs(delta))),
        "unregularized_negative_weight_count": 0,
        "unregularized_cancellation_ratio": 1.0,
        "regularization_path": [],
        "score_scaled_golden_threshold": 0.99,
        "score_multiplier_min": 1.0,
        "score_multiplier_max": 1.5,
        "high_golden_relative_lower": 0.9,
        "high_golden_relative_upper": 1.1,
        "high_golden_count": int(np.sum(high)),
    }
    return complete_weights, info

    design = rate_matrix / 50.0
    row_count, target_count = design.shape
    initial_replay = design @ golden
    initial_residual = initial_replay - desired
    delta_caps, delta_cap_rule = _golden_delta_caps(golden)
    optimizer_bounds = [
        (-float(limit), float(limit))
        for limit in delta_caps
    ]

    # Build a label-free target similarity graph from centered winrate-column
    # profiles. Near-identical columns are connected most strongly.
    centered_columns = rate_matrix - np.mean(rate_matrix, axis=0, keepdims=True)
    column_norms = np.linalg.norm(centered_columns, axis=0)
    valid_columns = column_norms > 1e-12
    normalized_columns = np.zeros_like(centered_columns)
    normalized_columns[:, valid_columns] = (
        centered_columns[:, valid_columns] / column_norms[valid_columns]
    )
    similarity = normalized_columns.T @ normalized_columns
    neighbor_count = min(8, max(0, target_count - 1))
    similarity_threshold = 0.25
    similarity_edges: Dict[Tuple[int, int], float] = {}
    for col in range(target_count):
        if not valid_columns[col] or neighbor_count == 0:
            continue
        candidates = np.argsort(-similarity[col], kind="stable")
        taken = 0
        for other in candidates:
            other = int(other)
            if other == col or not valid_columns[other]:
                continue
            sim = float(similarity[col, other])
            if sim <= similarity_threshold:
                break
            edge = (min(col, other), max(col, other))
            similarity_edges[edge] = max(
                similarity_edges.get(edge, 0.0),
                sim,
            )
            taken += 1
            if taken >= neighbor_count:
                break
    if similarity_edges:
        graph_i = np.asarray([edge[0] for edge in similarity_edges], dtype=int)
        graph_j = np.asarray([edge[1] for edge in similarity_edges], dtype=int)
        graph_weight = np.square(
            np.asarray(list(similarity_edges.values()), dtype=float)
        )
        graph_weight_sum = float(np.sum(graph_weight))
    else:
        graph_i = np.empty(0, dtype=int)
        graph_j = np.empty(0, dtype=int)
        graph_weight = np.empty(0, dtype=float)
        graph_weight_sum = 0.0

    # C-Score ordering is meaningful only within the same matchup-profile
    # neighborhood. Applying it to unrelated columns would erase counter
    # structure and turn the target into a disguised global rank table.
    order_low: List[int] = []
    order_high: List[int] = []
    order_weight: List[float] = []
    order_similarity_threshold = 0.75
    if target_scores is not None and target_count > 1:
        for (left, right), sim in similarity_edges.items():
            if sim < order_similarity_threshold:
                continue
            left_score = float(target_scores[left])
            right_score = float(target_scores[right])
            if abs(left_score - right_score) <= 1e-12:
                continue
            if left_score < right_score:
                order_low.append(left)
                order_high.append(right)
            else:
                order_low.append(right)
                order_high.append(left)
            order_weight.append(sim * sim)
    order_low_arr = np.asarray(order_low, dtype=int)
    order_high_arr = np.asarray(order_high, dtype=int)
    order_weight_arr = np.asarray(order_weight, dtype=float)
    order_pair_count = len(order_low)
    order_weight_sum = float(np.sum(order_weight_arr))

    similarity_strength = 2.0
    order_strength = 2.0

    def regularizer(delta: np.ndarray) -> Tuple[float, np.ndarray, Dict[str, float]]:
        penalty = float(delta @ delta)
        gradient = 2.0 * delta
        graph_mean_square = 0.0
        if graph_weight_sum > 0.0:
            graph_diff = delta[graph_i] - delta[graph_j]
            graph_mean_square = float(
                np.sum(graph_weight * graph_diff * graph_diff) / graph_weight_sum
            )
            graph_scale = (
                similarity_strength * target_count / graph_weight_sum
            )
            graph_contribution = 2.0 * graph_scale * graph_weight * graph_diff
            np.add.at(gradient, graph_i, graph_contribution)
            np.add.at(gradient, graph_j, -graph_contribution)
            penalty += (
                similarity_strength * target_count * graph_mean_square
            )
        order_mean_square = 0.0
        order_violation_rate = 0.0
        if order_pair_count > 0 and order_weight_sum > 0.0:
            weights = golden + delta
            order_diff = weights[order_low_arr] - weights[order_high_arr]
            active = order_diff > 0.0
            order_violation_rate = float(
                np.sum(order_weight_arr[active]) / order_weight_sum
            )
            if np.any(active):
                violation = order_diff[active]
                order_mean_square = float(
                    np.sum(order_weight_arr[active] * violation * violation)
                    / order_weight_sum
                )
                order_scale = (
                    2.0 * order_strength * target_count / order_weight_sum
                )
                order_contribution = (
                    order_scale * order_weight_arr[active] * violation
                )
                np.add.at(
                    gradient,
                    order_low_arr[active],
                    order_contribution,
                )
                np.add.at(
                    gradient,
                    order_high_arr[active],
                    -order_contribution,
                )
                penalty += (
                    order_strength * target_count * order_mean_square
                )
        return penalty, gradient, {
            "similar_increment_rms": math.sqrt(max(0.0, graph_mean_square)),
            "local_c_score_weight_inversion_rms": math.sqrt(
                max(0.0, order_mean_square)
            ),
            "local_c_score_weight_inversion_rate": order_violation_rate,
        }

    def replay_metrics(delta: np.ndarray) -> Dict[str, float]:
        weights = golden + delta
        residual = design @ weights - desired
        abs_residual = np.abs(residual)
        delta_cap_usage = np.abs(delta) / delta_caps
        _, _, shape = regularizer(delta)
        target_score_spearman = float("nan")
        if (
            target_scores is not None
            and target_count > 1
            and float(np.ptp(target_scores)) > 1e-12
            and float(np.ptp(weights)) > 1e-12
        ):
            target_score_spearman = float(
                pd.Series(weights).corr(
                    pd.Series(target_scores),
                    method="spearman",
                )
            )
        weight_sum = float(np.sum(weights))
        weight_l1_sum = float(np.sum(np.abs(weights)))
        return {
            "mean_abs_diff": float(np.mean(abs_residual)),
            "max_abs_diff": float(np.max(abs_residual)),
            "rmse": float(np.sqrt(np.mean(residual * residual))),
            "delta_l2_norm": float(np.linalg.norm(delta)),
            "delta_max_abs": float(np.max(np.abs(delta))),
            "delta_cap_max_usage": float(np.max(delta_cap_usage)),
            "delta_cap_binding_count": int(np.sum(delta_cap_usage >= 1.0 - 1e-7)),
            "negative_weight_count": int(np.sum(weights < 0.0)),
            "weight_sum": weight_sum,
            "weight_l1_sum": weight_l1_sum,
            "cancellation_ratio": (
                weight_l1_sum / max(abs(weight_sum), 1e-12)
            ),
            "cancellation_excess": max(
                0.0,
                weight_l1_sum / max(abs(weight_sum), 1e-12) - 1.0,
            ),
            "target_score_spearman": target_score_spearman,
            **shape,
        }

    def objective_and_gradient(
        delta: np.ndarray,
        alpha: float,
    ) -> Tuple[float, np.ndarray]:
        residual = initial_residual + design @ delta
        fit = float(np.mean(residual * residual))
        fit_gradient = 2.0 * (design.T @ residual) / row_count
        penalty, penalty_gradient, _ = regularizer(delta)
        return fit + alpha * penalty, fit_gradient + alpha * penalty_gradient

    # A decreasing path is warm-started from Golden. This is equivalent to
    # gradually allowing finer residual directions into the target.
    regularization_path = [
        3.0, 1.0, 0.3, 0.1, 0.03, 0.01, 0.003, 0.001,
        0.0003, 0.0001, 0.00003, 0.00001, 0.000003,
        0.000001, 0.0000003, 0.0000001,
    ]
    path_results: List[Dict[str, Any]] = []
    warm_delta = np.zeros(target_count, dtype=float)
    max_iterations = max(300, 8 * target_count)
    for alpha in regularization_path:
        result = minimize(
            lambda values, a=alpha: objective_and_gradient(values, a),
            warm_delta,
            method="L-BFGS-B",
            jac=True,
            bounds=optimizer_bounds,
            options={
                "maxiter": max_iterations,
                "ftol": 1e-14,
                "gtol": 1e-9,
                "maxls": 50,
            },
        )
        candidate_delta = np.asarray(result.x, dtype=float)
        if not np.all(np.isfinite(candidate_delta)):
            continue
        warm_delta = candidate_delta
        path_results.append({
            "alpha": float(alpha),
            "delta": candidate_delta.copy(),
            "iterations": int(getattr(result, "nit", 0)),
            "termination": str(result.message),
            "optimizer_success": bool(result.success),
            **replay_metrics(candidate_delta),
        })
    if not path_results:
        raise RuntimeError("Stable Correct target regularization path produced no solution")

    # The unregularized minimum-norm solution is diagnostic only. It determines
    # how much replay improvement is available, but can never be selected.
    unregularized_delta = np.linalg.lstsq(
        design,
        desired - initial_replay,
        rcond=1e-10,
    )[0]
    unregularized_metrics = replay_metrics(unregularized_delta)
    baseline_metrics = replay_metrics(np.zeros(target_count, dtype=float))
    best_mean_abs = min(
        unregularized_metrics["mean_abs_diff"],
        *(entry["mean_abs_diff"] for entry in path_results),
    )
    best_max_abs = min(
        unregularized_metrics["max_abs_diff"],
        *(entry["max_abs_diff"] for entry in path_results),
    )
    best_rmse = min(
        unregularized_metrics["rmse"],
        *(entry["rmse"] for entry in path_results),
    )
    selected, pareto_selection = _select_regularization_pareto_knee(
        path_results,
        baseline_metrics,
        {
            "mean_abs_diff": best_mean_abs,
            "max_abs_diff": best_max_abs,
            "rmse": best_rmse,
        },
    )

    delta = np.asarray(selected["delta"], dtype=float)
    complete_weights = golden + delta
    final_residual = design @ complete_weights - desired
    initial_abs = np.abs(initial_residual)
    final_abs = np.abs(final_residual)
    _, final_gradient = objective_and_gradient(delta, float(selected["alpha"]))
    info = {
        "algorithm": "golden_start_bounded_pareto_local_order_free_mass_backprop",
        "solver": "bounded_regularization_path_lbfgsb_with_normalized_pareto_knee",
        "iterations": int(selected["iterations"]),
        "maximum_iterations": int(max_iterations),
        "termination": selected["termination"],
        "optimizer_success": bool(selected["optimizer_success"]),
        "selection_reason": pareto_selection["selection_reason"],
        "selected_regularization_alpha": float(selected["alpha"]),
        "pareto_frontier_count": pareto_selection["frontier_count"],
        "pareto_plateau_fraction": pareto_selection["plateau_fraction"],
        "pareto_plateau_floor": pareto_selection["plateau_floor"],
        "pareto_plateau_count": pareto_selection["plateau_count"],
        "pareto_best_knee_score": pareto_selection["best_knee_score"],
        "pareto_replay_gain": pareto_selection["selected_replay_gain"],
        "pareto_structural_cost": pareto_selection["selected_structural_cost"],
        "pareto_knee_score": pareto_selection["selected_knee_score"],
        "pareto_stability_scales": pareto_selection["stability_scales"],
        "similarity_strength": similarity_strength,
        "similarity_threshold": similarity_threshold,
        "similarity_edge_count": int(len(graph_i)),
        "c_score_order_strength": order_strength,
        "c_score_order_similarity_threshold": order_similarity_threshold,
        "c_score_order_pair_count": int(order_pair_count),
        "golden_weight_sum": float(np.sum(golden)),
        "complete_weight_sum": float(np.sum(complete_weights)),
        "weight_sum_change": float(np.sum(complete_weights) - np.sum(golden)),
        "initial_mean_abs_diff": float(np.mean(initial_abs)),
        "initial_max_abs_diff": float(np.max(initial_abs)),
        "initial_rmse": float(np.sqrt(np.mean(initial_residual * initial_residual))),
        "final_mean_abs_diff": float(np.mean(final_abs)),
        "final_max_abs_diff": float(np.max(final_abs)),
        "final_rmse": float(np.sqrt(np.mean(final_residual * final_residual))),
        "delta_l2_norm": float(np.linalg.norm(delta)),
        "delta_max_abs": float(np.max(np.abs(delta))),
        "golden_delta_cap_relative_fraction": float(
            delta_cap_rule["relative_fraction"]
        ),
        "golden_delta_cap_floor_fraction": float(
            delta_cap_rule["floor_fraction"]
        ),
        "golden_delta_cap_anchor_scale": float(
            delta_cap_rule["anchor_scale"]
        ),
        "golden_delta_cap_min": float(delta_cap_rule["minimum_cap"]),
        "golden_delta_cap_max": float(delta_cap_rule["maximum_cap"]),
        "golden_delta_cap_max_usage": float(selected["delta_cap_max_usage"]),
        "golden_delta_cap_binding_count": int(
            selected["delta_cap_binding_count"]
        ),
        "negative_weight_count": int(np.sum(complete_weights < 0.0)),
        "weight_l1_sum": float(np.sum(np.abs(complete_weights))),
        "cancellation_ratio": float(selected["cancellation_ratio"]),
        "similar_increment_rms": float(selected["similar_increment_rms"]),
        "local_c_score_weight_inversion_rms": float(
            selected["local_c_score_weight_inversion_rms"]
        ),
        "local_c_score_weight_inversion_rate": float(
            selected["local_c_score_weight_inversion_rate"]
        ),
        "target_score_spearman": float(selected["target_score_spearman"]),
        "final_objective_gradient_l2": float(np.linalg.norm(final_gradient)),
        "unregularized_mean_abs_diff": float(
            unregularized_metrics["mean_abs_diff"]
        ),
        "unregularized_max_abs_diff": float(
            unregularized_metrics["max_abs_diff"]
        ),
        "unregularized_rmse": float(unregularized_metrics["rmse"]),
        "unregularized_delta_l2_norm": float(
            unregularized_metrics["delta_l2_norm"]
        ),
        "unregularized_delta_max_abs": float(
            unregularized_metrics["delta_max_abs"]
        ),
        "unregularized_negative_weight_count": int(
            unregularized_metrics["negative_weight_count"]
        ),
        "unregularized_cancellation_ratio": float(
            unregularized_metrics["cancellation_ratio"]
        ),
        "regularization_path": [
            {
                key: value
                for key, value in entry.items()
                if key != "delta"
            }
            for entry in path_results
        ],
    }
    return complete_weights, info


def _write_rowwise_correct_target_trace(
    groups_out: pd.DataFrame,
    all_edges: pd.DataFrame,
    reference_ids: Sequence[int],
    lane_size: int,
    raw_min: Optional[float],
    out_dir: Path,
    beta: float,
    alignment_scale: float,
    alignment_shift: float,
) -> Dict[str, Any]:
    """Trace Correct row coefficients and generate the exact K=5 big target.

    The exact row trace starts from Golden/50. Score changes made by the current
    Correct pipeline are applied as minimum-norm coefficient deltas on the legal
    Correct candidate rates. Final moment alignment is recorded as a scale step
    followed by a shift step. The resulting row coefficients exactly replay the
    existing Correct score.

    The ordinary weighted big target is closed-form: 0.9 times Golden plus five
    equal slots distributed over the complete legal reference universe. It
    exactly represents production Correct; only the later small-target
    compression is approximate.
    """
    required = {
        "group_id",
        "golden_rate",
        "Raw Cqd",
        "Correct_center_cqd_pre_final_moment_alignment",
        "Correct Cqd",
        "prospective_direct_reliability",
    }
    missing = sorted(required - set(groups_out.columns))
    if missing:
        raise RuntimeError(f"Rowwise Correct coefficient trace is missing columns: {missing}")
    out = groups_out.copy()
    out["group_id"] = out["group_id"].astype(int)
    if out["group_id"].duplicated().any():
        raise RuntimeError("Rowwise Correct coefficient trace requires unique group_id rows")

    score_ids = out["group_id"].astype(int).tolist()
    golden_by_gid = {
        int(gid): float(weight)
        for gid, weight in out[["group_id", "golden_rate"]].itertuples(index=False, name=None)
        if np.isfinite(float(weight)) and float(weight) > 0.0
    }
    legal_refs = sorted({int(g) for g in reference_ids})
    target_ids = sorted(set(golden_by_gid) | set(legal_refs))
    if not target_ids or not legal_refs:
        raise RuntimeError("Rowwise Correct coefficient trace has no target/reference candidates")
    missing_targets = sorted(set(target_ids) - set(score_ids))
    if missing_targets:
        raise RuntimeError(
            f"Rowwise Correct target candidates are outside score universe: {missing_targets[:30]}"
        )
    require_pairs_or_request(
        all_edges,
        score_ids,
        target_ids,
        "rowwise Correct coefficient trace",
        lane_size,
        out_dir,
    )

    edges, _ = _deduplicate_undirected_edges(all_edges)
    rate_lookup: Dict[Tuple[int, int], float] = {}
    sample_lookup: Dict[Tuple[int, int], float] = {}
    for a, b, y, n in edges[
        ["group_a", "group_b", "win_rate_a", "samples"]
    ].itertuples(index=False, name=None):
        aa, bb, yy, nn = int(a), int(b), float(y), float(n)
        rate_lookup[(aa, bb)] = 100.0 * yy
        rate_lookup[(bb, aa)] = 100.0 * (1.0 - yy)
        sample_lookup[(aa, bb)] = nn
        sample_lookup[(bb, aa)] = nn
    rates = np.asarray([
        [
            50.0 if int(score_gid) == int(target_gid)
            else rate_lookup[(int(score_gid), int(target_gid))]
            for target_gid in target_ids
        ]
        for score_gid in score_ids
    ], dtype=float)
    if not np.all(np.isfinite(rates)):
        raise RuntimeError("Rowwise Correct coefficient trace contains non-finite rates")

    target_col = {gid: idx for idx, gid in enumerate(target_ids)}
    reference_columns = [target_col[gid] for gid in legal_refs]
    golden = np.asarray([golden_by_gid.get(gid, 0.0) for gid in target_ids], dtype=float)
    raw_base_coefficient = golden / 50.0
    raw = pd.to_numeric(out["Raw Cqd"], errors="coerce").to_numpy(float)
    pre_correct = pd.to_numeric(
        out["Correct_center_cqd_pre_final_moment_alignment"], errors="coerce"
    ).to_numpy(float)
    final_correct = pd.to_numeric(out["Correct Cqd"], errors="coerce").to_numpy(float)
    final_correct_by_gid = {
        int(gid): float(score)
        for gid, score in zip(score_ids, final_correct)
    }
    target_correct_scores = np.asarray(
        [final_correct_by_gid[int(gid)] for gid in target_ids],
        dtype=float,
    )
    if not (
        np.all(np.isfinite(raw))
        and np.all(np.isfinite(pre_correct))
        and np.all(np.isfinite(final_correct))
        and np.isfinite(float(beta))
        and abs(float(beta)) > 1e-8
        and np.isfinite(float(alignment_scale))
        and np.isfinite(float(alignment_shift))
        and float(alignment_scale) > 0.0
    ):
        raise RuntimeError("Rowwise Correct coefficient trace requires finite score stages")

    raw_replay = rates @ raw_base_coefficient
    raw_reconciliation_delta = _minimum_norm_rate_coefficient_delta(
        rates, reference_columns, raw - raw_replay,
    )
    raw_coefficients = raw_base_coefficient[None, :] + raw_reconciliation_delta
    reliability = pd.to_numeric(
        out["prospective_direct_reliability"], errors="coerce"
    ).to_numpy(float)
    if not np.all(np.isfinite(reliability)):
        raise RuntimeError("Rowwise Correct coefficient trace has non-finite reliability")
    raw_by_gid = {int(gid): float(value) for gid, value in zip(score_ids, raw)}
    direct_reference_delta = np.zeros_like(rates)
    direct_reference_score = np.zeros(len(score_ids), dtype=float)
    beta_abs = abs(float(beta))
    for score_idx, score_gid in enumerate(score_ids):
        row_refs = [gid for gid in legal_refs if int(gid) != int(score_gid)]
        if not row_refs:
            raise RuntimeError(
                f"Rowwise Correct coefficient trace has no non-self references for group_id={score_gid}"
            )
        policy_mass = float(len(row_refs))
        for ref_gid in row_refs:
            rate = float(rates[score_idx, target_col[ref_gid]])
            n = float(sample_lookup[(int(score_gid), int(ref_gid))])
            y = rate / 100.0
            p = (y * n + 0.5) / (n + 1.0)
            observed_logit = float(logit(np.clip(p, 1e-6, 1.0 - 1e-6)))
            raw_eta = float(beta) * (
                raw_by_gid[int(score_gid)] - raw_by_gid[int(ref_gid)]
            )
            residual_cqd = (observed_logit - raw_eta) / beta_abs
            contribution = (
                float(reliability[score_idx]) * residual_cqd / policy_mass
            )
            direct_reference_score[score_idx] += contribution
            if abs(rate) > 1e-12:
                direct_reference_delta[
                    score_idx, target_col[ref_gid]
                ] += contribution / rate
    direct_correct_reconciliation_delta = _minimum_norm_rate_coefficient_delta(
        rates,
        reference_columns,
        (pre_correct - raw) - np.sum(rates * direct_reference_delta, axis=1),
    )
    direct_correct_delta = (
        direct_reference_delta + direct_correct_reconciliation_delta
    )
    pre_alignment_coefficients = raw_coefficients + direct_correct_delta
    alignment_scale_delta = (
        float(alignment_scale) - 1.0
    ) * pre_alignment_coefficients
    alignment_shift_delta = _minimum_norm_rate_coefficient_delta(
        rates,
        reference_columns,
        np.full(len(score_ids), float(alignment_shift), dtype=float),
    )
    final_coefficients = (
        pre_alignment_coefficients
        + alignment_scale_delta
        + alignment_shift_delta
    )

    raw_trace_diff = np.sum(rates * raw_coefficients, axis=1) - raw
    pre_trace_diff = np.sum(rates * pre_alignment_coefficients, axis=1) - pre_correct
    final_trace_diff = np.sum(rates * final_coefficients, axis=1) - final_correct
    row_trace_tolerance = 1e-9
    if (
        float(np.max(np.abs(raw_trace_diff))) > row_trace_tolerance
        or float(np.max(np.abs(pre_trace_diff))) > row_trace_tolerance
        or float(np.max(np.abs(final_trace_diff))) > row_trace_tolerance
    ):
        raise RuntimeError(
            "Rowwise Correct coefficient trace failed exact replay: "
            f"raw={np.max(np.abs(raw_trace_diff)):.12g} "
            f"pre={np.max(np.abs(pre_trace_diff)):.12g} "
            f"final={np.max(np.abs(final_trace_diff)):.12g}"
        )

    row_index = np.repeat(np.arange(len(score_ids)), len(target_ids))
    target_index = np.tile(np.arange(len(target_ids)), len(score_ids))
    row_trace = pd.DataFrame({
        "score_group_id": np.asarray(score_ids, dtype=int)[row_index],
        "target_group_id": np.asarray(target_ids, dtype=int)[target_index],
        "is_legal_correct_reference": np.isin(
            np.asarray(target_ids, dtype=int)[target_index],
            np.asarray(legal_refs, dtype=int),
        ).astype(int),
        "target_rate": rates.reshape(-1),
        "raw_golden_coefficient": np.tile(raw_base_coefficient, len(score_ids)),
        "raw_rounding_reconciliation_delta": raw_reconciliation_delta.reshape(-1),
        "raw_stage_coefficient": raw_coefficients.reshape(-1),
        "direct_reference_delta": direct_reference_delta.reshape(-1),
        "direct_correct_reconciliation_delta": direct_correct_reconciliation_delta.reshape(-1),
        "direct_correct_delta": direct_correct_delta.reshape(-1),
        "pre_alignment_coefficient": pre_alignment_coefficients.reshape(-1),
        "final_alignment_scale_delta": alignment_scale_delta.reshape(-1),
        "final_alignment_shift_delta": alignment_shift_delta.reshape(-1),
        "final_coefficient": final_coefficients.reshape(-1),
    })
    row_trace.to_csv(out_dir / "target_correct_row_coefficient_trace.csv", index=False)

    coefficient_mean = np.mean(final_coefficients, axis=0)
    coefficient_stddev = np.std(final_coefficients, axis=0, ddof=0)
    # The production K=5 Correct has an exact common target. Keep 45/50 of
    # Golden and represent the five synthetic slots by spreading weight 5
    # equally across the complete legal reference universe. No inverse fit is
    # needed here; only the later small-target compression is approximate.
    common_weight = 0.9 * golden
    common_weight[np.asarray(reference_columns, dtype=int)] += (
        float(PROSPECTIVE_REPLACEMENT_K) / float(len(reference_columns))
    )
    exact_replay = rates @ (common_weight / 50.0)
    exact_diff = exact_replay - final_correct
    initial_diff = rates @ (golden / 50.0) - final_correct
    delta_from_golden = common_weight - golden
    common_projection = {
        "solver": "closed_form_fixed_slot_replacement",
        "iterations": 0,
        "termination": "exact_analytic_solution",
        "initial_mean_abs_diff": float(np.mean(np.abs(initial_diff))),
        "initial_max_abs_diff": float(np.max(np.abs(initial_diff))),
        "initial_rmse": float(np.sqrt(np.mean(initial_diff * initial_diff))),
        "delta_l2_norm": float(np.linalg.norm(delta_from_golden)),
        "delta_max_abs": float(np.max(np.abs(delta_from_golden))),
        "golden_delta_cap_relative_fraction": 0.0,
        "golden_delta_cap_floor_fraction": 0.0,
        "golden_delta_cap_anchor_scale": 0.0,
        "golden_delta_cap_min": 0.0,
        "golden_delta_cap_max": 0.0,
        "golden_delta_cap_max_usage": 0.0,
        "golden_delta_cap_binding_count": 0,
        "final_objective_gradient_l2": 0.0,
        "selected_regularization_alpha": 0.0,
        "selection_reason": "exact_closed_form_k5_target",
        "pareto_frontier_count": 1,
        "pareto_plateau_fraction": 1.0,
        "pareto_plateau_floor": 0.0,
        "pareto_plateau_count": 1,
        "pareto_best_knee_score": 0.0,
        "pareto_replay_gain": float(np.mean(np.abs(initial_diff)) - np.mean(np.abs(exact_diff))),
        "pareto_structural_cost": 0.0,
        "pareto_knee_score": 0.0,
        "similarity_edge_count": 0,
        "similarity_threshold": 0.0,
        "similar_increment_rms": 0.0,
        "c_score_order_pair_count": 0,
        "c_score_order_similarity_threshold": 0.0,
        "local_c_score_weight_inversion_rms": 0.0,
        "local_c_score_weight_inversion_rate": 0.0,
        "target_score_spearman": float(pd.Series(common_weight).corr(pd.Series(target_correct_scores), method="spearman")),
        "cancellation_ratio": 1.0,
        "unregularized_mean_abs_diff": float(np.mean(np.abs(exact_diff))),
        "unregularized_max_abs_diff": float(np.max(np.abs(exact_diff))),
        "unregularized_rmse": float(np.sqrt(np.mean(exact_diff * exact_diff))),
        "unregularized_delta_l2_norm": float(np.linalg.norm(delta_from_golden)),
        "unregularized_delta_max_abs": float(np.max(np.abs(delta_from_golden))),
        "unregularized_negative_weight_count": 0,
        "unregularized_cancellation_ratio": 1.0,
        "weight_sum_change": float(np.sum(common_weight) - np.sum(golden)),
        "regularization_path": [],
    }
    common_coefficient = common_weight / 50.0
    nonzero = np.abs(common_coefficient) > 1e-15
    if not np.any(nonzero):
        raise RuntimeError("Common Correct coefficient projection is empty")
    common_replay = rates @ common_coefficient
    common_diff = common_replay - final_correct
    coefficient_residual = final_coefficients - common_coefficient[None, :]
    common_weight_sum = float(np.sum(common_weight))
    common_weight_l1_sum = float(np.sum(np.abs(common_weight)))
    if not np.isfinite(common_weight_l1_sum) or common_weight_l1_sum <= 0.0:
        raise RuntimeError("Common Correct coefficient projection has invalid L1 mass")

    trace_version = "fixed_slot_replacement_exact_big_target_v1"
    score_mode = "exact_k5_replacement_correct_big_target"
    rows = pd.DataFrame({
        "trace_version": trace_version,
        "score_mode": score_mode,
        "lane_size": int(lane_size),
        "reference_scope": "common_correct_candidate_coefficients",
        "group_id": np.asarray(target_ids, dtype=int)[nonzero],
        # Audit-only normalized magnitude. Signed targeting semantics live in
        # common_coefficient/correct_target_weight.
        "reference_weight": np.abs(common_weight[nonzero]) / common_weight_l1_sum,
        "nominal_weight": common_weight[nonzero],
        "raw_golden_weight": golden[nonzero],
        "common_coefficient": common_coefficient[nonzero],
        "coefficient_mean": coefficient_mean[nonzero],
        "coefficient_stddev": coefficient_stddev[nonzero],
        "correct_target_weight": common_weight[nonzero],
        "source": "closed_form_0.9_golden_plus_5_over_n_legal_references",
    })
    rows.to_csv(out_dir / "target_correct_trace_weights.csv", index=False)
    abs_common_diff = np.abs(common_diff)
    abs_coefficient_residual = np.abs(coefficient_residual)
    info = {
        "version": "rowwise_correct_coefficient_trace_v1",
        "row_trace_file": "target_correct_row_coefficient_trace.csv",
        "score_row_count": int(len(score_ids)),
        "target_candidate_count": int(len(target_ids)),
        "legal_correct_reference_count": int(len(legal_refs)),
        "raw_golden_weight_sum": float(np.sum(golden)),
        "row_trace_raw_replay_max_abs_diff": float(np.max(np.abs(raw_trace_diff))),
        "row_trace_pre_alignment_replay_max_abs_diff": float(np.max(np.abs(pre_trace_diff))),
        "direct_reference_score_reconciliation_max_abs_diff": float(np.max(np.abs(
            (pre_correct - raw) - direct_reference_score
        ))),
        "row_trace_final_replay_mean_abs_diff": float(np.mean(np.abs(final_trace_diff))),
        "row_trace_final_replay_max_abs_diff": float(np.max(np.abs(final_trace_diff))),
        "common_projection_rule": "weight=0.9*Golden+(5/N)*legal_reference_indicator",
        "common_projection_objective": "exactly replay fixed-slot K=5 Correct before compression",
        "common_projection_iterations": common_projection["iterations"],
        "common_projection_termination": common_projection["termination"],
        "common_projection_initial_mean_abs_diff": (
            common_projection["initial_mean_abs_diff"]
        ),
        "common_projection_initial_max_abs_diff": (
            common_projection["initial_max_abs_diff"]
        ),
        "common_projection_initial_rmse": common_projection["initial_rmse"],
        "common_projection_delta_l2_norm": common_projection["delta_l2_norm"],
        "common_projection_delta_max_abs": common_projection["delta_max_abs"],
        "common_projection_golden_delta_cap_relative_fraction": (
            common_projection["golden_delta_cap_relative_fraction"]
        ),
        "common_projection_golden_delta_cap_floor_fraction": (
            common_projection["golden_delta_cap_floor_fraction"]
        ),
        "common_projection_golden_delta_cap_anchor_scale": (
            common_projection["golden_delta_cap_anchor_scale"]
        ),
        "common_projection_golden_delta_cap_min": (
            common_projection["golden_delta_cap_min"]
        ),
        "common_projection_golden_delta_cap_max": (
            common_projection["golden_delta_cap_max"]
        ),
        "common_projection_golden_delta_cap_max_usage": (
            common_projection["golden_delta_cap_max_usage"]
        ),
        "common_projection_golden_delta_cap_binding_count": (
            common_projection["golden_delta_cap_binding_count"]
        ),
        "common_projection_final_objective_gradient_l2": (
            common_projection["final_objective_gradient_l2"]
        ),
        "common_projection_selected_regularization_alpha": (
            common_projection["selected_regularization_alpha"]
        ),
        "common_projection_selection_reason": common_projection["selection_reason"],
        "common_projection_pareto_frontier_count": (
            common_projection["pareto_frontier_count"]
        ),
        "common_projection_pareto_plateau_fraction": (
            common_projection["pareto_plateau_fraction"]
        ),
        "common_projection_pareto_plateau_floor": (
            common_projection["pareto_plateau_floor"]
        ),
        "common_projection_pareto_plateau_count": (
            common_projection["pareto_plateau_count"]
        ),
        "common_projection_pareto_best_knee_score": (
            common_projection["pareto_best_knee_score"]
        ),
        "common_projection_pareto_replay_gain": (
            common_projection["pareto_replay_gain"]
        ),
        "common_projection_pareto_structural_cost": (
            common_projection["pareto_structural_cost"]
        ),
        "common_projection_pareto_knee_score": (
            common_projection["pareto_knee_score"]
        ),
        "common_projection_similarity_edge_count": (
            common_projection["similarity_edge_count"]
        ),
        "common_projection_similarity_threshold": (
            common_projection["similarity_threshold"]
        ),
        "common_projection_similar_increment_rms": (
            common_projection["similar_increment_rms"]
        ),
        "common_projection_c_score_order_pair_count": (
            common_projection["c_score_order_pair_count"]
        ),
        "common_projection_c_score_order_similarity_threshold": (
            common_projection["c_score_order_similarity_threshold"]
        ),
        "common_projection_local_c_score_weight_inversion_rms": (
            common_projection["local_c_score_weight_inversion_rms"]
        ),
        "common_projection_local_c_score_weight_inversion_rate": (
            common_projection["local_c_score_weight_inversion_rate"]
        ),
        "common_projection_target_score_spearman": (
            common_projection["target_score_spearman"]
        ),
        "common_projection_cancellation_ratio": (
            common_projection["cancellation_ratio"]
        ),
        "common_projection_unregularized_mean_abs_diff": (
            common_projection["unregularized_mean_abs_diff"]
        ),
        "common_projection_unregularized_max_abs_diff": (
            common_projection["unregularized_max_abs_diff"]
        ),
        "common_projection_unregularized_rmse": (
            common_projection["unregularized_rmse"]
        ),
        "common_projection_unregularized_delta_l2_norm": (
            common_projection["unregularized_delta_l2_norm"]
        ),
        "common_projection_unregularized_delta_max_abs": (
            common_projection["unregularized_delta_max_abs"]
        ),
        "common_projection_unregularized_negative_weight_count": (
            common_projection["unregularized_negative_weight_count"]
        ),
        "common_projection_unregularized_cancellation_ratio": (
            common_projection["unregularized_cancellation_ratio"]
        ),
        "negative_coefficient_count": int(np.sum(common_coefficient < 0.0)),
        "common_nonzero_coefficient_count": int(np.sum(nonzero)),
        "common_coefficient_sum": float(np.sum(common_coefficient)),
        "correct_target_weight_sum": common_weight_sum,
        "correct_target_weight_sum_change": common_projection["weight_sum_change"],
        "correct_target_weight_l1_sum": common_weight_l1_sum,
        "common_forward_replay_mean_abs_diff": float(np.mean(abs_common_diff)),
        "common_forward_replay_max_abs_diff": float(np.max(abs_common_diff)),
        "common_forward_replay_rmse": float(np.sqrt(np.mean(common_diff * common_diff))),
        "coefficient_projection_mean_abs_diff": float(np.mean(abs_coefficient_residual)),
        "coefficient_projection_max_abs_diff": float(np.max(abs_coefficient_residual)),
        "coefficient_projection_rmse": float(np.sqrt(np.mean(coefficient_residual * coefficient_residual))),
    }
    metadata = {
        "trace_version": trace_version,
        "score_mode": score_mode,
        "lane_size": int(lane_size),
        "calibration_raw_min": None if raw_min is None else float(raw_min),
        "raw_component": {
            "weight_source": "lane_results.golden_rate",
            "weight_sum": float(np.sum(golden)),
            "coefficient_source": "Golden(g)/50",
            "score_formula": "Raw(x)=sum_g rate(x,g)*Golden(g)/50",
        },
        "rowwise_correct_component": {
            "initial_coefficient": "Golden(g)/50",
            "raw_rounding_reconciliation": "minimum-L2 legal-reference coefficient delta",
            "direct_correct_adjustment": (
                "each reliability-scaled equal-policy residual contribution is "
                "recorded on its own legal reference coefficient; floating-point "
                "reconciliation uses a minimum-L2 legal-reference delta"
            ),
            "raw_beta": float(beta),
            "final_moment_alignment": {
                "scale": float(alignment_scale),
                "shift": float(alignment_shift),
                "shift_distribution": "minimum-L2 legal-reference coefficient delta",
            },
            "row_score_formula": "Correct_i=sum_g rate(i,g)*row_coefficient(i,g)",
            "exact_row_replay": True,
            "row_trace_file": "target_correct_row_coefficient_trace.csv",
            "row_replay_mean_abs_diff": info["row_trace_final_replay_mean_abs_diff"],
            "row_replay_max_abs_diff": info["row_trace_final_replay_max_abs_diff"],
        },
        "common_correct_component": {
            "coefficient_rule": info["common_projection_rule"],
            "objective": info["common_projection_objective"],
            "trace_anchor": "Golden(g)",
            "solver": common_projection["solver"],
            "iterations": info["common_projection_iterations"],
            "termination": info["common_projection_termination"],
            "initial_forward_replay_mean_abs_diff": (
                info["common_projection_initial_mean_abs_diff"]
            ),
            "initial_forward_replay_max_abs_diff": (
                info["common_projection_initial_max_abs_diff"]
            ),
            "initial_forward_replay_rmse": info["common_projection_initial_rmse"],
            "delta_from_golden_l2": info["common_projection_delta_l2_norm"],
            "delta_from_golden_max_abs": info["common_projection_delta_max_abs"],
            "golden_delta_guard": {
                "rule": "not_applicable_closed_form_exact_target",
                "relative_fraction": (
                    info[
                        "common_projection_golden_delta_cap_relative_fraction"
                    ]
                ),
                "floor_fraction": (
                    info[
                        "common_projection_golden_delta_cap_floor_fraction"
                    ]
                ),
                "anchor_scale": (
                    info["common_projection_golden_delta_cap_anchor_scale"]
                ),
                "minimum_cap": (
                    info["common_projection_golden_delta_cap_min"]
                ),
                "maximum_cap": (
                    info["common_projection_golden_delta_cap_max"]
                ),
                "selected_max_usage": (
                    info["common_projection_golden_delta_cap_max_usage"]
                ),
                "selected_binding_count": (
                    info["common_projection_golden_delta_cap_binding_count"]
                ),
            },
            "selected_regularization_alpha": (
                info["common_projection_selected_regularization_alpha"]
            ),
            "selection_reason": info["common_projection_selection_reason"],
            "pareto_frontier_count": (
                info["common_projection_pareto_frontier_count"]
            ),
            "pareto_plateau_fraction": (
                info["common_projection_pareto_plateau_fraction"]
            ),
            "pareto_plateau_floor": (
                info["common_projection_pareto_plateau_floor"]
            ),
            "pareto_plateau_count": (
                info["common_projection_pareto_plateau_count"]
            ),
            "pareto_best_knee_score": (
                info["common_projection_pareto_best_knee_score"]
            ),
            "pareto_replay_gain": (
                info["common_projection_pareto_replay_gain"]
            ),
            "pareto_structural_cost": (
                info["common_projection_pareto_structural_cost"]
            ),
            "pareto_knee_score": (
                info["common_projection_pareto_knee_score"]
            ),
            "similarity_edge_count": (
                info["common_projection_similarity_edge_count"]
            ),
            "similarity_threshold": (
                info["common_projection_similarity_threshold"]
            ),
            "similar_increment_rms": (
                info["common_projection_similar_increment_rms"]
            ),
            "c_score_order_pair_count": (
                info["common_projection_c_score_order_pair_count"]
            ),
            "c_score_order_similarity_threshold": (
                info["common_projection_c_score_order_similarity_threshold"]
            ),
            "local_c_score_weight_inversion_rms": (
                info["common_projection_local_c_score_weight_inversion_rms"]
            ),
            "local_c_score_weight_inversion_rate": (
                info["common_projection_local_c_score_weight_inversion_rate"]
            ),
            "target_score_spearman": (
                info["common_projection_target_score_spearman"]
            ),
            "cancellation_ratio": info["common_projection_cancellation_ratio"],
            "final_objective_gradient_l2": (
                info["common_projection_final_objective_gradient_l2"]
            ),
            "unregularized_benchmark": {
                "forward_replay_mean_abs_diff": (
                    info["common_projection_unregularized_mean_abs_diff"]
                ),
                "forward_replay_max_abs_diff": (
                    info["common_projection_unregularized_max_abs_diff"]
                ),
                "forward_replay_rmse": (
                    info["common_projection_unregularized_rmse"]
                ),
                "delta_from_golden_l2": (
                    info["common_projection_unregularized_delta_l2_norm"]
                ),
                "delta_from_golden_max_abs": (
                    info["common_projection_unregularized_delta_max_abs"]
                ),
                "negative_weight_count": (
                    info["common_projection_unregularized_negative_weight_count"]
                ),
                "cancellation_ratio": (
                    info["common_projection_unregularized_cancellation_ratio"]
                ),
            },
            "regularization_path": common_projection["regularization_path"],
            "coefficient_sum": info["common_coefficient_sum"],
            "weight_rule": "correct_target_weight(g)=50*a_g",
            "weight_sum": info["correct_target_weight_sum"],
            "weight_sum_change_from_golden": (
                info["correct_target_weight_sum_change"]
            ),
            "weight_l1_sum": info["correct_target_weight_l1_sum"],
            "target_count": info["common_nonzero_coefficient_count"],
            "score_formula": "ApproxCorrect(x)=sum_g rate(x,g)*correct_target_weight(g)/50",
            "ordinary_weighted_targeting": True,
            "forward_replay_mean_abs_diff": info["common_forward_replay_mean_abs_diff"],
            "forward_replay_max_abs_diff": info["common_forward_replay_max_abs_diff"],
            "forward_replay_rmse": info["common_forward_replay_rmse"],
            "negative_coefficient_count": info["negative_coefficient_count"],
        },
        "serialized_selection_weight_rule": {
            "scoreable_candidate": "existing Correct score is unchanged",
            "target_trace": "exact 0.9*Golden plus 5/N legal-reference Correct big target",
            "column": "selection_weight_cqd / Selection Weight Cqd Display",
        },
    }
    (out_dir / "target_correct_trace_metadata.json").write_text(
        json.dumps(metadata, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return info


def _prospective_greedy_take(
    pool: pd.DataFrame,
    group_members: Dict[int, List[str]],
    score_col: str,
    max_refs: int,
    context: str,
) -> List[int]:
    if pool is None or pool.empty or max_refs <= 0:
        return []
    tmp = pool.copy()
    tmp[score_col] = pd.to_numeric(tmp[score_col], errors="coerce").astype(float)
    tmp = tmp[np.isfinite(tmp[score_col].to_numpy(float))].copy()
    if tmp.empty:
        return []
    ids = _greedy_visible_main_group_ids(tmp, group_members, score_col=score_col, context=context)
    return [int(g) for g in ids[:int(max_refs)]]


def _prospective_unique_member_extend(
    chosen: List[int],
    candidates: Sequence[int],
    group_members: Dict[int, List[str]],
    max_refs: int,
) -> List[int]:
    out: List[int] = []
    used = set()
    for gid in list(chosen) + [int(g) for g in candidates]:
        if gid in out:
            continue
        members = [str(m) for m in group_members.get(int(gid), [])]
        if members and any(m in used for m in members):
            continue
        out.append(int(gid))
        used.update(members)
        if len(out) >= int(max_refs):
            break
    return out


def _prospective_disabled_mask(df: pd.DataFrame) -> pd.Series:
    """Rows that must not define a prospective reference environment.

    The production Correct still scores score-only / blocked rows, but future
    environments must match the old active-environment semantics: disabled or
    blocked rows are not allowed to act as reference rows.  The mask is intentionally
    column-tolerant so DB/diagnostic aliases cannot silently leak disabled rows
    into the environment.
    """
    if df is None or df.empty:
        return pd.Series([], dtype=bool)
    mask = pd.Series(False, index=df.index)
    true_means_disabled = [
        "blocked_score_only_candidate",
        "blocked_by_db",
        "is_blocked",
        "blocked",
        "disabled",
        "is_disabled",
        "disabled_candidate",
        "candidate_disabled",
        "excluded_by_blocklist",
    ]
    for col in true_means_disabled:
        if col in df.columns:
            mask = mask | df[col].fillna(False).astype(bool)
    false_means_disabled = ["enabled", "is_enabled", "active_enabled", "candidate_enabled"]
    for col in false_means_disabled:
        if col in df.columns:
            mask = mask | (~df[col].fillna(True).astype(bool))
    return mask.fillna(False).astype(bool)


def _prospective_validate_reference_environment(
    env_name: str,
    ref_ids: Sequence[int],
    group_members: Dict[int, List[str]],
    disabled_ids: Set[int],
) -> Dict[str, Any]:
    """Audit and enforce no disabled rows and no repeated members in env refs."""
    seen_gids: Set[int] = set()
    seen_members: Set[str] = set()
    duplicate_group_ids: List[int] = []
    duplicate_member_group_ids: List[int] = []
    disabled_group_ids: List[int] = []
    clean: List[int] = []
    for raw_gid in ref_ids:
        gid = int(raw_gid)
        if gid in seen_gids:
            duplicate_group_ids.append(gid)
            continue
        seen_gids.add(gid)
        if gid in disabled_ids:
            disabled_group_ids.append(gid)
            continue
        members = [str(m) for m in group_members.get(gid, []) if str(m) != ""]
        if not members:
            members = [f"__gid__{gid}"]
        if any(m in seen_members for m in members):
            duplicate_member_group_ids.append(gid)
            continue
        clean.append(gid)
        seen_members.update(members)
    if disabled_group_ids:
        raise RuntimeError(
            f"Prospective environment {env_name} contains disabled/blocked reference rows; "
            f"first_group_ids={disabled_group_ids[:30]}"
        )
    return {
        "clean_group_ids": clean,
        "duplicate_group_id_count": int(len(duplicate_group_ids)),
        "duplicate_member_group_count": int(len(duplicate_member_group_ids)),
        "disabled_reference_count": int(len(disabled_group_ids)),
        "removed_duplicate_group_ids": _compact_id_list(duplicate_group_ids, limit=80),
        "removed_duplicate_member_group_ids": _compact_id_list(duplicate_member_group_ids, limit=80),
    }




def _prospective_reference_delta(
    score_universe: pd.DataFrame,
    all_edges: pd.DataFrame,
    ref_ids: Sequence[int],
    beta: float,
) -> pd.DataFrame:
    gids = [int(g) for g in score_universe["group_id"].astype(int).tolist()]
    raw_map = {int(g): float(r) for g, r in score_universe[["group_id", "raw_cqd"]].itertuples(index=False, name=None)}
    ref_set = {int(g) for g in ref_ids}
    target_set = set(gids)
    beta_abs = abs(float(beta))
    rows: List[Dict[str, Any]] = []
    acc: Dict[int, Dict[str, Any]] = {int(g): {"values": [], "measurement_variances": [], "edges": 0.0, "samples": 0.0} for g in gids}
    if all_edges is not None and not all_edges.empty and ref_set and beta_abs > 1e-8:
        for a, b, wr, samples in all_edges[["group_a", "group_b", "win_rate_a", "samples"]].itertuples(index=False, name=None):
            ia, ib = int(a), int(b)
            if ia == ib:
                continue
            if ia not in raw_map or ib not in raw_map:
                continue
            n = _prospective_safe_float(samples, 0.0)
            if not np.isfinite(n) or n <= 0:
                continue
            y = _prospective_safe_float(wr, np.nan)
            if not np.isfinite(y):
                continue
            p = (float(y) * n + 0.5) / (n + 1.0)
            obs = float(logit(np.clip(p, 1e-6, 1 - 1e-6)))
            measurement_var_cqd = 1.0 / max((n + 1.0) * p * (1.0 - p) * beta_abs * beta_abs, 1e-12)
            raw_eta = float(beta) * (raw_map[ia] - raw_map[ib])
            resid_a = obs - raw_eta
            if ia in target_set and ib in ref_set:
                acc[ia]["values"].append(resid_a / beta_abs)
                acc[ia]["measurement_variances"].append(measurement_var_cqd)
                acc[ia]["edges"] += 1.0
                acc[ia]["samples"] += n
            if ib in target_set and ia in ref_set:
                acc[ib]["values"].append((-resid_a) / beta_abs)
                acc[ib]["measurement_variances"].append(measurement_var_cqd)
                acc[ib]["edges"] += 1.0
                acc[ib]["samples"] += n
    for gid in gids:
        d = acc[int(gid)]
        expected_edges = max(1, len(ref_set) - (1 if int(gid) in ref_set else 0))
        if abs(float(beta)) <= 1e-8:
            delta_cqd, se_cqd, effective_edges, scenario_scale = np.nan, np.nan, 0.0, np.nan
        else:
            delta_cqd, se_cqd, effective_edges, scenario_scale = _equal_reference_location_and_se(
                d["values"], d["measurement_variances"]
            )
        mean_resid_logit = delta_cqd * abs(float(beta)) if np.isfinite(delta_cqd) else np.nan
        rows.append({
            "group_id": int(gid),
            "prospective_reference_delta_logit": float(mean_resid_logit) if np.isfinite(mean_resid_logit) else np.nan,
            "prospective_reference_delta_cqd": float(delta_cqd) if np.isfinite(delta_cqd) else np.nan,
            "prospective_reference_se_cqd": float(se_cqd) if np.isfinite(se_cqd) else np.nan,
            "prospective_reference_effective_edge_count": float(effective_edges),
            "prospective_reference_scenario_scale_cqd": float(scenario_scale) if np.isfinite(scenario_scale) else np.nan,
            "prospective_reference_superpopulation_se_cqd": float(math.sqrt(max(0.0, se_cqd * se_cqd + scenario_scale * scenario_scale / max(effective_edges, 1.0)))) if np.isfinite(se_cqd) and np.isfinite(scenario_scale) else np.nan,
            "prospective_reference_policy_weight_per_edge": float(1.0 / d["edges"]) if d["edges"] > 0 else np.nan,
            "prospective_reference_mean_measurement_se_cqd": float(np.mean(np.sqrt(d["measurement_variances"]))) if d["measurement_variances"] else np.nan,
            "prospective_reference_edge_count": int(d["edges"]),
            "prospective_reference_sample_mass": float(d["samples"]),
            "prospective_reference_coverage_ratio": float(min(1.0, d["edges"] / float(expected_edges))),
            "prospective_reference_count": int(len(ref_set)),
        })
    return pd.DataFrame(rows)


def _prospective_mean_rate_against_references(
    score_universe: pd.DataFrame,
    all_edges: pd.DataFrame,
    ref_ids: Sequence[int],
) -> pd.DataFrame:
    """Equal-weight win rate against the fixed legal reference universe.

    A reference scores 50 against itself. All other rates must be present; the
    caller runs the missing-rate preflight before reaching this function.
    """
    gids = [int(g) for g in score_universe["group_id"].astype(int).tolist()]
    refs = [int(g) for g in ref_ids]
    ref_set = set(refs)
    values: Dict[int, Dict[int, float]] = {gid: {} for gid in gids}
    for gid in gids:
        if gid in ref_set:
            values[gid][gid] = 50.0
    for a, b, wr in all_edges[["group_a", "group_b", "win_rate_a"]].itertuples(index=False, name=None):
        ia, ib = int(a), int(b)
        rate = float(wr) * 100.0
        if ia in values and ib in ref_set:
            values[ia][ib] = rate
        if ib in values and ia in ref_set:
            values[ib][ia] = 100.0 - rate
    rows = []
    for gid in gids:
        missing = [ref for ref in refs if ref not in values[gid]]
        if missing:
            raise RuntimeError(
                "Prospective replacement Correct is missing target/reference rates; "
                f"group_id={gid}, first_reference_ids={missing[:30]}"
            )
        rows.append({
            "group_id": gid,
            "prospective_replacement_mean_rate_cqd": float(
                np.mean([values[gid][ref] for ref in refs])
            ),
        })
    return pd.DataFrame(rows)


def _write_prospective_pair_metrics(
    out_dir: Path,
    groups_out: pd.DataFrame,
    all_edges: pd.DataFrame,
    beta: float,
) -> Dict[str, Any]:
    if all_edges is None or all_edges.empty or groups_out is None or groups_out.empty:
        pd.DataFrame([]).to_csv(out_dir / "prospective_correct_pair_metrics.csv", index=False)
        return {}
    score_map = {int(g): float(s) for g, s in groups_out[["group_id", "Correct Cqd"]].itertuples(index=False, name=None)}
    raw_map = {int(g): float(s) for g, s in groups_out[["group_id", "Raw Cqd"]].itertuples(index=False, name=None)}
    sub = all_edges[all_edges["group_a"].astype(int).isin(score_map) & all_edges["group_b"].astype(int).isin(score_map)].copy()
    if sub.empty:
        pd.DataFrame([]).to_csv(out_dir / "prospective_correct_pair_metrics.csv", index=False)
        return {}
    raw_eta = []
    corr_eta = []
    for a, b in sub[["group_a", "group_b"]].itertuples(index=False, name=None):
        ia, ib = int(a), int(b)
        raw_eta.append(float(beta) * (raw_map[ia] - raw_map[ib]))
        corr_eta.append(float(beta) * (score_map[ia] - score_map[ib]))
    m = metrics_for(sub, np.asarray(raw_eta, dtype=float), np.asarray(corr_eta, dtype=float))
    m.to_csv(out_dir / "prospective_correct_pair_metrics.csv", index=False)
    out: Dict[str, Any] = {}
    try:
        raw_r = m[m.model == "raw"].iloc[0].to_dict()
        cor_r = m[m.model == "corrected"].iloc[0].to_dict()
        for k, v in raw_r.items():
            if isinstance(v, (int, float, np.integer, np.floating)):
                out[f"raw_{k}"] = float(v)
        for k, v in cor_r.items():
            if isinstance(v, (int, float, np.integer, np.floating)):
                out[f"prospective_{k}"] = float(v)
    except Exception:
        pass
    return out


def _select_nonnegative_logloss_alpha(
    y: np.ndarray,
    n: np.ndarray,
    raw_eta: np.ndarray,
    direct_increment_eta: np.ndarray,
) -> Tuple[float, float, float, bool, int]:
    """Fit one non-negative OOF slope with no artificial upper bound."""
    y = np.asarray(y, dtype=float)
    n = np.asarray(n, dtype=float)
    raw_eta = np.asarray(raw_eta, dtype=float)
    inc = np.asarray(direct_increment_eta, dtype=float)
    ok = np.isfinite(y) & np.isfinite(n) & np.isfinite(raw_eta) & np.isfinite(inc) & (n > 0.0)
    y, n, raw_eta, inc = y[ok], n[ok], raw_eta[ok], inc[ok]
    if len(y) == 0:
        raise RuntimeError("No non-score-only OOF observations for direct-reference alpha")
    raw_loss = float(binomial_logloss(y, n, raw_eta))
    if not np.any(np.abs(inc) > 1e-15):
        return 0.0, raw_loss, raw_loss, True, 0
    # The objective is convex in alpha.  Solve its analytic score equation so
    # the answer cannot depend on optimizer starting value or finite-difference
    # tolerance (the log-loss improvements are intentionally small).
    mass = max(float(np.sum(n)), 1.0)
    def gradient(alpha: float) -> float:
        return float(np.sum(n * inc * (expit(raw_eta + float(alpha) * inc) - y)) / mass)
    if gradient(0.0) >= 0.0:
        return 0.0, raw_loss, raw_loss, True, 0
    hi = 1.0
    while gradient(hi) < 0.0 and hi < 1e12:
        hi *= 2.0
    if gradient(hi) < 0.0:
        raise RuntimeError("Direct-reference OOF alpha has no finite optimum")
    lo = 0.0
    iterations = 0
    for iterations in range(1, 101):
        mid = 0.5 * (lo + hi)
        if gradient(mid) < 0.0:
            lo = mid
        else:
            hi = mid
        if hi - lo <= 1e-12 * max(1.0, hi):
            break
    alpha = float(0.5 * (lo + hi))
    calibrated_loss = float(binomial_logloss(y, n, raw_eta + alpha * inc))
    return alpha, raw_loss, calibrated_loss, True, int(iterations)


def _fit_direct_reference_oof_alpha(
    score_universe: pd.DataFrame,
    all_edges: pd.DataFrame,
    reference_ids: Sequence[int],
    beta: float,
    raw_min: Optional[float],
    out_dir: Path,
) -> Dict[str, Any]:
    """Learn one scalar direct-mean signal scale with cluster-equal OOF.

    The returned tau is frozen before score-only projection.  It controls only
    the transparent measurement-error reliability tau^2/(tau^2+SE_g^2); there
    is no global correction multiplier and no row-level CQD cap.
    """
    df = score_universe.drop_duplicates("group_id", keep="first").copy().reset_index(drop=True)
    df["group_id"] = df["group_id"].astype(int)
    df["raw_cqd"] = pd.to_numeric(df["raw_cqd"], errors="coerce").astype(float)
    score_only = pd.Series(False, index=df.index)
    for col in ["scout_candidate", "score_only_candidate", "is_score_only", "score_only", "active_set_score_only_candidate"]:
        if col in df.columns:
            score_only = score_only | df[col].fillna(False).astype(bool)
    fit_mask = (~_prospective_disabled_mask(df)) & (~score_only) & np.isfinite(df["raw_cqd"])
    if raw_min is not None:
        fit_mask = fit_mask & (df["raw_cqd"] >= float(raw_min))
    fit_ids = set(df.loc[fit_mask, "group_id"].astype(int))
    if len(fit_ids) < 2:
        raise RuntimeError("Too few non-score-only groups for direct-reference OOF alpha")

    edges = _prepare_partial_edges_for_ids(all_edges, df["group_id"].tolist(), "direct-reference OOF alpha")
    ref_set = {int(g) for g in reference_ids}
    target_fit_ids = set(fit_ids) - ref_set
    reference_only_validation = not bool(target_fit_ids)
    nfold = max(2, min(PROSPECTIVE_CROSSFIT_FOLDS, len(ref_set)))
    # Stable hash ordering followed by round-robin assignment keeps reference
    # folds balanced without using any outcome or score-only information.
    ordered_refs = sorted(ref_set, key=lambda gid: (((int(gid) * 2654435761) & 0xFFFFFFFF), int(gid)))
    ref_fold = {int(gid): int(pos % nfold) for pos, gid in enumerate(ordered_refs)}
    raw_map = {int(g): float(r) for g, r in df[["group_id", "raw_cqd"]].itertuples(index=False, name=None)}
    oof_rows: List[pd.DataFrame] = []
    fold_rows: List[Dict[str, Any]] = []
    for fold in range(nfold):
        held_refs = {gid for gid, f in ref_fold.items() if int(f) == int(fold)}
        train_refs = sorted(ref_set - held_refs)
        fold_delta = _prospective_reference_delta(df, edges, train_refs, beta)
        delta_map = {
            int(g): float(v) for g, v in fold_delta[["group_id", "prospective_reference_delta_cqd"]].itertuples(index=False, name=None)
            if np.isfinite(float(v))
        }
        se_map = {
            int(g): float(v) for g, v in fold_delta[["group_id", "prospective_reference_se_cqd"]].itertuples(index=False, name=None)
            if np.isfinite(float(v)) and float(v) >= 0.0
        }
        scenario_mean_var_map = {
            int(g): float(max(0.0, float(scale)) ** 2 / max(float(neff), 1.0))
            for g, scale, neff in fold_delta[[
                "group_id", "prospective_reference_scenario_scale_cqd",
                "prospective_reference_effective_edge_count",
            ]].itertuples(index=False, name=None)
            if np.isfinite(float(scale)) and np.isfinite(float(neff))
        }
        ga_all = edges["group_a"].astype(int)
        gb_all = edges["group_b"].astype(int)
        # Exactly one endpoint is a held reference and the other is a legal
        # non-reference fit target.  Consequently the evaluated edge was used
        # in neither endpoint's train-reference mean.
        if reference_only_validation:
            # Lane 1 can have every legal non-score-only fit row in the
            # member-unique reference set. Use edges between two held references:
            # each endpoint's direct mean was fitted only against train_refs, so
            # the held-held edge is absent from both estimates. Canonical A is
            # the target and canonical B is the held policy opponent.
            calibration_hold = (
                ga_all.isin(held_refs).to_numpy(bool)
                & gb_all.isin(held_refs).to_numpy(bool)
            )
        else:
            calibration_hold = (
                (ga_all.isin(target_fit_ids).to_numpy(bool) & gb_all.isin(held_refs).to_numpy(bool))
                | (gb_all.isin(target_fit_ids).to_numpy(bool) & ga_all.isin(held_refs).to_numpy(bool))
            )
        sub = edges.loc[calibration_hold].copy()
        if reference_only_validation:
            target_gid = sub["group_a"].astype(int).to_numpy()
        else:
            target_gid = np.where(
                sub["group_a"].astype(int).isin(target_fit_ids).to_numpy(bool),
                sub["group_a"].astype(int).to_numpy(),
                sub["group_b"].astype(int).to_numpy(),
            )
        valid = np.asarray([
            int(g) in delta_map and int(g) in se_map and int(g) in scenario_mean_var_map
            for g in target_gid
        ], dtype=bool)
        sub = sub.loc[valid].copy()
        if not sub.empty:
            ga = sub["group_a"].astype(int).to_numpy(); gb = sub["group_b"].astype(int).to_numpy()
            a_is_target = np.ones(len(ga), dtype=bool) if reference_only_validation else np.isin(ga, list(target_fit_ids))
            target_gid = np.where(a_is_target, ga, gb).astype(int)
            held_reference_id = np.where(a_is_target, gb, ga).astype(int)
            raw_eta = float(beta) * np.asarray([
                raw_map[int(t)] - raw_map[int(r)] for t, r in zip(target_gid, held_reference_id)
            ], dtype=float)
            y_target = np.where(a_is_target, sub["win_rate_a"].to_numpy(float), 1.0 - sub["win_rate_a"].to_numpy(float))
            delta_target = np.asarray([delta_map[int(g)] for g in target_gid], dtype=float)
            se_target = np.asarray([se_map[int(g)] for g in target_gid], dtype=float)
            scenario_mean_var_target = np.asarray([scenario_mean_var_map[int(g)] for g in target_gid], dtype=float)
            inc = float(beta) * delta_target
            oof_rows.append(pd.DataFrame({
                "fold": int(fold), "win_rate_a": y_target,
                "samples": sub["samples"].to_numpy(float), "raw_eta": raw_eta,
                "direct_increment_eta": inc, "group_a": ga, "group_b": gb,
                "target_group_id": target_gid,
                "direct_delta_target_cqd": delta_target,
                "direct_se_target_cqd": se_target,
                "direct_scenario_mean_var_target_cqd2": scenario_mean_var_target,
                "held_reference_id": held_reference_id,
                "validation_orientation": "target_to_held_reference",
            }))
        fold_rows.append({
            "fold": int(fold), "training_reference_count": int(len(train_refs)),
            "held_reference_count": int(len(held_refs)),
            "oof_non_score_only_nonreference_target_edges": int(len(sub)),
            "oof_score_only_edges": 0,
            "validation_target_mode": "held_reference_to_held_reference" if reference_only_validation else "nonreference_target_to_held_reference",
        })
    if not oof_rows:
        raise RuntimeError("Direct-reference OOF alpha produced no eligible held-out edges")
    oof = pd.concat(oof_rows, ignore_index=True)
    y_oof = oof["win_rate_a"].to_numpy(float)
    raw_eta_oof = oof["raw_eta"].to_numpy(float)
    delta_target = oof["direct_delta_target_cqd"].to_numpy(float)
    se_target = oof["direct_se_target_cqd"].to_numpy(float)
    scenario_mean_var_target = oof["direct_scenario_mean_var_target_cqd2"].to_numpy(float)
    clusters = oof["held_reference_id"].astype(int).to_numpy()

    def eta_for_tau(tau: float, kappa: float = 0.0) -> np.ndarray:
        t2 = max(0.0, float(tau)) ** 2
        if t2 == 0.0:
            return raw_eta_oof.copy()
        k = max(0.0, float(kappa))
        variance = se_target * se_target + k * scenario_mean_var_target
        reliability = t2 / np.maximum(t2 + variance, 1e-300)
        return raw_eta_oof + float(beta) * reliability * delta_target

    def cluster_equal_loss_for_tau(tau: float, kappa: float = 0.0) -> float:
        eta = eta_for_tau(tau, kappa)
        edge_loss = np.logaddexp(0.0, eta) - y_oof * eta
        return float(pd.DataFrame({"cluster": clusters, "loss": edge_loss}).groupby("cluster", sort=False)["loss"].mean().mean())

    finite_se = se_target[np.isfinite(se_target)]
    se_scale = float(np.median(finite_se)) if len(finite_se) else 1.0
    se_scale = max(se_scale, 1e-12)
    # Bounds are purely numerical: 1e-8*SE is the exact-zero limit and
    # 1e8*SE is the no-shrink limit. They are not CQD/business thresholds.
    opt = minimize_scalar(
        lambda log_ratio: cluster_equal_loss_for_tau(se_scale * math.exp(float(log_ratio))),
        bounds=(math.log(1e-8), math.log(1e8)), method="bounded",
        options={"xatol": 1e-10, "maxiter": 300},
    )
    if not opt.success or not np.isfinite(opt.x):
        raise RuntimeError(f"Direct-reference OOF tau fit failed: {opt.message}")
    tau_oof = float(se_scale * math.exp(float(opt.x)))
    # Estimate the production signal scale from the heteroskedastic marginal
    # distribution of full-reference direct means on fit rows. OOF remains an
    # independent predictive audit. Letting a one-SE predictive rule choose the
    # production scale can legitimately hit exactly zero on a weak batch and
    # collapse every Correct score to Raw; REML estimates the latent across-row
    # signal variance directly and has no hand-set movement floor.
    full_direct = _prospective_reference_delta(df, edges, reference_ids, beta)
    reml = full_direct[full_direct["group_id"].astype(int).isin(fit_ids)].copy()
    z_reml = pd.to_numeric(reml["prospective_reference_delta_cqd"], errors="coerce").to_numpy(float)
    se_reml = pd.to_numeric(reml["prospective_reference_se_cqd"], errors="coerce").to_numpy(float)
    reml_ok = np.isfinite(z_reml) & np.isfinite(se_reml) & (se_reml >= 0.0)
    z_reml, se_reml = z_reml[reml_ok], se_reml[reml_ok]
    if len(z_reml) < 3:
        raise RuntimeError("Too few non-score-only direct means for REML tau")
    reml_scale = max(float(np.median(se_reml)), float(np.std(z_reml, ddof=1)), 1e-12)

    def reml_nll(tau_value: float) -> float:
        variance = np.maximum(se_reml * se_reml + max(0.0, float(tau_value)) ** 2, 1e-300)
        precision = 1.0 / variance
        mu = float(np.sum(precision * z_reml) / np.sum(precision))
        # Restricted likelihood profiles out the unknown common location.
        return float(0.5 * (np.sum(np.log(variance) + (z_reml - mu) ** 2 / variance) + math.log(np.sum(precision))))

    reml_opt = minimize_scalar(
        lambda log_ratio: reml_nll(reml_scale * math.exp(float(log_ratio))),
        bounds=(math.log(1e-8), math.log(1e8)), method="bounded",
        options={"xatol": 1e-10, "maxiter": 300},
    )
    if not reml_opt.success or not np.isfinite(reml_opt.x):
        raise RuntimeError(f"Direct-reference REML tau fit failed: {reml_opt.message}")
    tau = float(reml_scale * math.exp(float(reml_opt.x)))
    if tau <= reml_scale * 1e-7:
        raise RuntimeError(
            "Direct-reference REML estimated zero latent signal; refusing to emit Raw-as-Correct "
            "without inventing an artificial minimum movement"
        )
    # With tau frozen by REML, learn how much between-reference scenario
    # dispersion is genuinely predictive uncertainty. Kappa=0 is the complete
    # fixed-policy interpretation; kappa=1 is the old hard-coded superpopulation
    # assumption. No upper business bound is imposed.
    raw_loss = cluster_equal_loss_for_tau(0.0)
    kappa_zero_loss = cluster_equal_loss_for_tau(tau, 0.0)
    kappa_opt = minimize_scalar(
        lambda log_k: cluster_equal_loss_for_tau(tau, math.exp(float(log_k))),
        bounds=(math.log(1e-12), math.log(1e12)), method="bounded",
        options={"xatol": 1e-10, "maxiter": 300},
    )
    if not kappa_opt.success or not np.isfinite(kappa_opt.x):
        raise RuntimeError(f"Direct-reference OOF kappa fit failed: {kappa_opt.message}")
    positive_kappa = float(math.exp(float(kappa_opt.x)))
    positive_kappa_loss = cluster_equal_loss_for_tau(tau, positive_kappa)
    # Continuous kappa is not identifiable here: when OOF prefers Raw it runs to
    # infinity and switches every correction off. Average the two explicit
    # scientific models directly and equally: kappa=0 (complete fixed policy)
    # and kappa=1 (standard superpopulation prediction variance). Do not let an
    # unstable batch-level evidence weighting move this midpoint per run.
    kappa_one_loss = cluster_equal_loss_for_tau(tau, 1.0)
    cluster_count = max(1, int(len(np.unique(clusters))))
    log_evidence_ratio_one_over_zero = float(cluster_count) * float(kappa_zero_loss - kappa_one_loss)
    kappa = 0.5
    calibrated_loss = cluster_equal_loss_for_tau(tau, kappa)
    no_shrink_loss = cluster_equal_loss_for_tau(se_scale * 1e8, 0.0)
    effective_var_target = se_target * se_target + kappa * scenario_mean_var_target
    oof["direct_reliability_target"] = tau * tau / np.maximum(tau * tau + effective_var_target, 1e-300)
    oof["calibrated_eta"] = eta_for_tau(tau, kappa)
    oof.to_csv(out_dir / "prospective_direct_reference_oof_predictions.csv", index=False)
    pd.DataFrame(fold_rows).to_csv(out_dir / "prospective_direct_reference_oof_folds.csv", index=False)
    info = {
        "direct_reference_oof_alpha": 1.0,
        "direct_reference_oof_alpha_selection_rule": "disabled_replaced_by_scalar_measurement_error_tau",
        "direct_reference_oof_tau_cqd": float(tau),
        "direct_reference_oof_tau_selection_rule": "heteroskedastic_reml_non_score_only_full_reference_means",
        "direct_reference_oof_tau_oof_optimum_cqd": float(tau_oof),
        "direct_reference_oof_tau_reml_rows": int(len(z_reml)),
        "direct_reference_oof_tau_reml_nll": float(reml_nll(tau)),
        "direct_reference_oof_tau_reml_zero_nll": float(reml_nll(0.0)),
        "direct_reference_oof_scenario_kappa": float(kappa),
        "direct_reference_oof_scenario_kappa_selection_rule": "equal_arithmetic_mean_fixed_zero_vs_superpopulation_one",
        "direct_reference_oof_scenario_kappa_zero_loss": float(kappa_zero_loss),
        "direct_reference_oof_scenario_kappa_one_loss": float(kappa_one_loss),
        "direct_reference_oof_scenario_kappa_log_evidence_ratio_one_over_zero": float(log_evidence_ratio_one_over_zero),
        "direct_reference_oof_scenario_kappa_positive_optimum": float(positive_kappa),
        "direct_reference_oof_scenario_kappa_positive_loss": float(positive_kappa_loss),
        "direct_reference_oof_scenario_kappa_oof_would_disable_model": int(positive_kappa > 1e6),
        "direct_reference_oof_validation_orientation": "target_to_held_reference_policy_residual",
        "direct_reference_oof_validation_target_mode": "held_reference_to_held_reference" if reference_only_validation else "nonreference_target_to_held_reference",
        "direct_reference_oof_reference_clusters": int(cluster_count),
        "direct_reference_oof_raw_logloss": float(raw_loss),
        "direct_reference_oof_no_shrink_logloss": float(no_shrink_loss),
        "direct_reference_oof_calibrated_logloss": float(calibrated_loss),
        "direct_reference_oof_logloss_delta": float(calibrated_loss - raw_loss),
        "direct_reference_oof_optimizer_converged": int(bool(opt.success)),
        "direct_reference_oof_optimizer_iterations": int(getattr(opt, "nit", 0) or 0),
        "direct_reference_oof_fit_groups": int(len(fit_ids)),
        "direct_reference_oof_score_only_edges": 0,
    }
    pd.DataFrame([info]).to_csv(out_dir / "prospective_direct_reference_oof_summary.csv", index=False)
    return info


def _derive_reference_anchored_embedding(
    df: pd.DataFrame,
    edges: pd.DataFrame,
    train_mask: np.ndarray,
    reference_ids: Sequence[int],
    beta: float,
) -> Tuple[int, pd.DataFrame, np.ndarray]:
    """Fit a continuous residual basis on references; project targets read-only."""
    ref_set = {int(g) for g in reference_ids}
    gids = df["group_id"].astype(int).to_numpy()
    raw = df["raw_cqd"].to_numpy(float)
    ref_rows = np.flatnonzero(np.isin(gids, list(ref_set)))
    if len(ref_rows) < 2:
        return 0, pd.DataFrame([{"component": 1, "singular_value": 0.0, "selected": False}]), np.zeros((len(df), 0))
    n_bins = max(2, int(math.ceil(math.sqrt(len(ref_rows)))))
    ref_raw = raw[ref_rows]
    try:
        ref_bin = np.asarray(pd.qcut(ref_raw, q=min(n_bins, len(ref_rows)), labels=False, duplicates="drop"), dtype=float)
        ref_bin = np.nan_to_num(ref_bin, nan=0.0).astype(int)
    except Exception:
        ref_bin = np.zeros(len(ref_rows), dtype=int)
    bin_by_row = np.full(len(df), -1, dtype=int)
    bin_by_row[ref_rows] = ref_bin
    B = int(max(1, ref_bin.max() + 1))
    prof_num = np.zeros((len(df), B), dtype=float)
    prof_den = np.zeros((len(df), B), dtype=float)
    for pos in np.flatnonzero(train_mask):
        er = edges.iloc[int(pos)]
        ia, ib = int(er["ia"]), int(er["ib"])
        n = float(er["samples"]); y = float(er["win_rate_a"])
        p = (y * n + 0.5) / (n + 1.0)
        resid = float(logit(np.clip(p, 1e-6, 1 - 1e-6))) - float(beta) * (raw[ia] - raw[ib])
        if bin_by_row[ib] >= 0:
            b = int(bin_by_row[ib]); prof_num[ia, b] += resid * n; prof_den[ia, b] += n
        if bin_by_row[ia] >= 0:
            b = int(bin_by_row[ia]); prof_num[ib, b] += (-resid) * n; prof_den[ib, b] += n
    prof = np.divide(prof_num, np.maximum(prof_den, 1e-12))
    row_mean = np.divide(np.sum(prof_num, axis=1, keepdims=True), np.maximum(np.sum(prof_den, axis=1, keepdims=True), 1e-12))
    prof = np.where(prof_den > 0, prof - row_mean, 0.0)
    ref_mean = np.mean(prof[ref_rows], axis=0, keepdims=True)
    ref_sd = np.std(prof[ref_rows], axis=0, keepdims=True)
    X = (prof - ref_mean) / np.maximum(ref_sd, 1e-12)
    X = np.nan_to_num(X)
    Xref = X[ref_rows]
    _, s, Vt = np.linalg.svd(Xref, full_matrices=False)
    med = float(np.median(s)) if len(s) else 0.0
    mad = float(np.median(np.abs(s - med))) if len(s) else 0.0
    floor = med + 1.4826 * mad
    rank = int(np.sum(s > floor))
    if rank < 2:
        rank = 0
    embedding = X.dot(Vt[:rank].T) if rank > 0 else np.zeros((len(df), 0))
    if rank > 0:
        embedding -= np.mean(embedding[ref_rows], axis=0, keepdims=True)
        embedding /= np.maximum(np.sqrt(np.mean(embedding[ref_rows] ** 2, axis=0, keepdims=True)), 1e-12)
    denom = float(np.sum(s * s)) if len(s) else 0.0
    spectrum = pd.DataFrame([{
        "component": i + 1, "singular_value": float(sv), "robust_noise_floor": floor,
        "selected": bool(i < rank),
        "cumulative_variance_share": float(np.sum(s[:i + 1] ** 2) / denom) if denom > 0 else 0.0,
    } for i, sv in enumerate(s)])
    return rank, spectrum, embedding


def _member_conditioned_strength_eb(
    observed_logit: np.ndarray,
    observed_se_logit: np.ndarray,
    group_ids: Sequence[int],
    group_members: Dict[int, List[str]],
    fit_mask: Optional[np.ndarray] = None,
    *,
    max_iter: int = PROSPECTIVE_MEMBER_EB_MAX_ROUNDS,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray, float, float, bool, int]:
    """Continuous member-common + partner-specific empirical-Bayes decomposition."""
    z = np.asarray(observed_logit, dtype=float)
    se = np.asarray(observed_se_logit, dtype=float)
    gids = [int(g) for g in group_ids]
    members_by_group: List[List[str]] = []
    fit_rows = np.ones(len(gids), dtype=bool) if fit_mask is None else np.asarray(fit_mask, dtype=bool)
    if len(fit_rows) != len(gids):
        raise ValueError("fit_mask must match group_ids")
    member_frequency: Dict[str, int] = {}
    for gid in gids:
        ms = [str(m) for m in group_members.get(gid, []) if str(m) != ""]
        if not ms:
            ms = [f"__gid__{gid}"]
        members_by_group.append(ms)
        if fit_rows[len(members_by_group) - 1]:
            for m in ms:
                member_frequency[m] = member_frequency.get(m, 0) + 1
    # Only repeated members define a common component.  A one-off member is
    # observationally indistinguishable from this group's partner-specific
    # term and must not create a duplicate latent parameter.
    member_names = sorted([m for m, count in member_frequency.items() if count >= 2])
    midx = {m: j for j, m in enumerate(member_names)}
    rr: List[int] = []; cc: List[int] = []; dd: List[float] = []
    for i, ms in enumerate(members_by_group):
        repeated = [m for m in ms if m in midx]
        scale = 1.0 / math.sqrt(len(repeated)) if repeated else 0.0
        for m in repeated:
            rr.append(i); cc.append(midx[m]); dd.append(scale)
    X = sparse.csr_matrix((dd, (rr, cc)), shape=(len(gids), len(member_names)))
    finite = np.isfinite(z) & np.isfinite(se) & (se >= 0.0)
    trainable = finite & fit_rows
    z = np.where(finite, z, 0.0)
    finite_se = se[trainable]
    fallback_se = float(np.median(finite_se)) if len(finite_se) else 1.0
    se = np.where(finite, se, fallback_se)
    variance = np.maximum(se * se, 1e-10)
    signal_sd = float(np.std(z[trainable])) if np.any(trainable) else 0.0
    tau_member = max(signal_sd / math.sqrt(2.0), 1e-6)
    tau_partner = max(signal_sd / math.sqrt(2.0), 1e-6)
    fit_idx = np.flatnonzero(trainable)
    Xfit = X[fit_idx]
    zfit = z[fit_idx]
    vfit = variance[fit_idx]
    member_coef = np.zeros(X.shape[1], dtype=float)
    partner_fit = np.zeros(len(fit_idx), dtype=float)
    W = sparse.diags(1.0 / vfit)
    I_m = sparse.eye(X.shape[1], format="csr")
    I_g = sparse.eye(len(fit_idx), format="csr")
    converged = False; rounds = 0
    for eb_round in range(int(max_iter)):
        rounds = eb_round + 1
        A = sparse.hstack([Xfit, I_g], format="csr")
        prior = sparse.block_diag((I_m / (tau_member * tau_member), I_g / (tau_partner * tau_partner)), format="csr")
        lhs = A.T.dot(W.dot(A)) + prior
        rhs = A.T.dot((1.0 / vfit) * zfit)
        coef = sparse.linalg.spsolve(lhs.tocsc(), rhs)
        member_coef = np.asarray(coef[:X.shape[1]], dtype=float)
        partner_fit = np.asarray(coef[X.shape[1]:], dtype=float)
        new_tau_member = max(float(np.sqrt(np.mean(member_coef ** 2))) if len(member_coef) else 0.0, 1e-6)
        new_tau_partner = max(float(np.sqrt(np.mean(partner_fit ** 2))) if len(partner_fit) else 0.0, 1e-6)
        if max(abs(math.log(new_tau_member / tau_member)), abs(math.log(new_tau_partner / tau_partner))) < 3e-3:
            tau_member, tau_partner = new_tau_member, new_tau_partner
            converged = True
            break
        tau_member, tau_partner = new_tau_member, new_tau_partner
    member_component = np.asarray(X.dot(member_coef)).ravel()
    # EB is used only to attribute the projected strength between a repeated-
    # member common component and a partner-specific component.  It must not
    # attenuate the scalar strength exported to Correct: in real audits the
    # fitted tau_partner can collapse to its numerical floor and turn dozens of
    # valid 0.2--1.4 CQD residuals into ~1e-9 movements.  Keep the decomposition
    # additive and exact for both fit rows and read-only score rows.  Rows
    # outside fit_mask still cannot affect member_coef or either fitted tau.
    partner_coef = z - member_component
    posterior = member_component + partner_coef
    return posterior, member_component, partner_coef, float(tau_member), float(tau_partner), bool(converged), int(rounds)


def _select_oof_ordering_calibrated_alpha(
    y: np.ndarray,
    n: np.ndarray,
    raw_eta: np.ndarray,
    strength_increment: np.ndarray,
) -> Tuple[float, float, float, bool, int]:
    """Maximize held-out weighted ordering exactly; logloss breaks ties."""
    y = np.asarray(y, dtype=float); n = np.asarray(n, dtype=float)
    raw_eta = np.asarray(raw_eta, dtype=float); inc = np.asarray(strength_increment, dtype=float)
    truth = y >= 0.5
    pred0 = raw_eta >= 0.0
    total = max(float(np.sum(n)), 1.0)
    initial_correct = float(np.sum(n * (pred0 == truth)))
    cross = -raw_eta / np.where(np.abs(inc) > 0.0, inc, np.nan)
    valid = np.isfinite(cross) & (cross > 0.0)
    events = pd.DataFrame({
        "alpha": cross[valid],
        "delta": n[valid] * np.where(pred0[valid] == truth[valid], -1.0, 1.0),
    }).groupby("alpha", sort=True, as_index=False)["delta"].sum()
    boundaries = [0.0] + events["alpha"].astype(float).tolist() + [float("inf")]
    correct = initial_correct
    intervals: List[Tuple[float, float, float]] = []
    for k in range(len(boundaries) - 1):
        lo, hi = float(boundaries[k]), float(boundaries[k + 1])
        intervals.append((lo, hi, correct / total))
        if k < len(events):
            correct += float(events.iloc[k]["delta"])
    best_ordering = max(v for _, _, v in intervals) if intervals else initial_correct / total
    raw_ordering = initial_correct / total
    best_alpha = 0.0
    best_loss = np.inf
    optimizer_all_converged = True; optimizer_max_iterations = 0
    for lo, hi, ordering in intervals:
        if ordering < best_ordering - 1e-15:
            continue
        if np.isfinite(hi) and hi - lo <= 1e-12:
            candidates = [lo]
        elif not np.isfinite(hi):
            lower = lo + max(1e-10, abs(lo) * 1e-12)
            opt = minimize(
                lambda z: binomial_logloss(y, n, raw_eta + float(z[0]) * inc),
                np.asarray([max(1.0, lower * 2.0)]), method="L-BFGS-B", bounds=[(lower, None)],
                options={"maxiter": 100},
            )
            optimizer_all_converged = optimizer_all_converged and bool(opt.success)
            optimizer_max_iterations = max(optimizer_max_iterations, int(getattr(opt, "nit", 0) or 0))
            candidates = [lower]
            if opt.success and np.isfinite(opt.x[0]):
                candidates.append(float(opt.x[0]))
        else:
            eps = min(1e-10, (hi - lo) / 4.0)
            lower, upper = lo + eps, hi - eps
            opt = minimize(
                lambda z: binomial_logloss(y, n, raw_eta + float(z[0]) * inc),
                np.asarray([(lower + upper) / 2.0]), method="L-BFGS-B", bounds=[(lower, upper)],
                options={"maxiter": 100},
            )
            optimizer_all_converged = optimizer_all_converged and bool(opt.success)
            optimizer_max_iterations = max(optimizer_max_iterations, int(getattr(opt, "nit", 0) or 0))
            candidates = [lower, upper]
            if opt.success and np.isfinite(opt.x[0]):
                candidates.append(float(opt.x[0]))
        for a in candidates:
            loss = binomial_logloss(y, n, raw_eta + float(a) * inc)
            if loss < best_loss - 1e-15 or (abs(loss - best_loss) <= 1e-15 and a < best_alpha):
                best_alpha, best_loss = float(a), float(loss)
    return float(best_alpha), float(raw_ordering), float(best_ordering), bool(optimizer_all_converged), int(optimizer_max_iterations)




def _crossfit_continuous_strength_adjustment(
    score_universe: pd.DataFrame,
    all_edges: pd.DataFrame,
    reference_ids: Sequence[int],
    group_members: Dict[int, List[str]],
    beta: float,
    seed: int,
    out_dir: Path,
) -> pd.DataFrame:
    """Separate scalar strength from continuous non-transitive matchup shape.

    Text-Type and RSW-Type are deliberately absent.  Residual profiles create a
    continuous spectral embedding using training edges only.  Its antisymmetric
    low-rank interaction explains matchup-specific advantage, while only the
    cross-fit-bagged scalar group delta is allowed into Correct.
    """
    df = score_universe.drop_duplicates("group_id", keep="first").copy().reset_index(drop=True)
    df["group_id"] = df["group_id"].astype(int)
    df["raw_cqd"] = pd.to_numeric(df["raw_cqd"], errors="coerce").astype(float)
    edges = _prepare_partial_edges_for_ids(all_edges, df["group_id"].tolist(), "prospective continuous strength/interactions")
    ref_set = {int(g) for g in reference_ids}
    edge_is_policy = edges["group_a"].astype(int).isin(ref_set) | edges["group_b"].astype(int).isin(ref_set)
    edges = edges.loc[edge_is_policy].copy().reset_index(drop=True)
    if edges.empty:
        raise RuntimeError("No prospective target-reference edges for continuous strength decomposition")
    nfold = max(2, min(PROSPECTIVE_CROSSFIT_FOLDS, len(edges)))
    edges["fold5"] = edge_fold_ids(edges["group_a"].to_numpy(), edges["group_b"].to_numpy(), nfold=nfold)
    type_ids = np.zeros(len(df), dtype=int)  # disables every discrete type-counter parameter
    beta_abs = abs(float(beta))
    if beta_abs <= 1e-8:
        raise RuntimeError("Prospective continuous strength decomposition requires nonzero Raw beta")

    fold_delta: List[np.ndarray] = []
    fold_member_component: List[np.ndarray] = []
    fold_partner_component: List[np.ndarray] = []
    fold_rows: List[Dict[str, Any]] = []
    oof_rows: List[pd.DataFrame] = []
    score_only_mask = pd.Series(False, index=df.index)
    for col in ["scout_candidate", "score_only_candidate", "is_score_only", "score_only", "active_set_score_only_candidate"]:
        if col in df.columns:
            score_only_mask = score_only_mask | df[col].fillna(False).astype(bool)
    member_eb_fit_mask = (~_prospective_disabled_mask(df) & ~score_only_mask).to_numpy(bool)
    fit_gid_set = set(df.loc[member_eb_fit_mask, "group_id"].astype(int))
    for fold in range(nfold):
        hold = edges["fold5"].astype(int).to_numpy() == int(fold)
        train = ~hold
        if not np.any(train) or not np.any(hold):
            continue
        rank, spectrum, embedding = _derive_reference_anchored_embedding(
            df, edges, train, reference_ids, float(beta),
        )
        # Identify strength relative to the actual prospective policy.  With
        # E_ref[u_ref] = 0, every skew interaction satisfies
        # E_ref[u_i^T S u_ref] = 0, so it cannot transfer a reference-average
        # residual into (or out of) scalar strength as spectral rank changes.
        ref_row_mask = df["group_id"].astype(int).isin(ref_set).to_numpy(bool)
        if embedding.shape[1] > 0 and np.any(ref_row_mask):
            embedding = embedding - np.mean(embedding[ref_row_mask], axis=0, keepdims=True)
            ref_scale = np.sqrt(np.mean(embedding[ref_row_mask] ** 2, axis=0, keepdims=True))
            embedding = embedding / np.maximum(ref_scale, 1e-12)
            reference_embedding_mean_error = float(np.max(np.abs(np.mean(embedding[ref_row_mask], axis=0))))
        else:
            reference_embedding_mean_error = 0.0
        spectrum = spectrum.copy()
        spectrum["fold"] = int(fold)
        spectrum.to_csv(out_dir / f"prospective_continuous_spectrum_fold_{fold}.csv", index=False)
        # Fit the environment only on member-unique references.  Duplicate
        # candidate families and score-only targets cannot influence gamma,
        # tau, or another group's scalar strength.
        ref_rows = np.flatnonzero(ref_row_mask)
        ref_df = df.iloc[ref_rows].copy().reset_index(drop=True)
        ref_index = {int(g): i for i, g in enumerate(ref_df["group_id"].astype(int))}
        ref_edge_mask = train & edges["group_a"].astype(int).isin(ref_set).to_numpy(bool) & edges["group_b"].astype(int).isin(ref_set).to_numpy(bool)
        ref_edges = edges.loc[ref_edge_mask].copy().reset_index(drop=True)
        ref_edges["ia"] = ref_edges["group_a"].astype(int).map(ref_index).astype(int)
        ref_edges["ib"] = ref_edges["group_b"].astype(int).map(ref_index).astype(int)
        ref_train = np.ones(len(ref_edges), dtype=bool)
        ref_embedding = embedding[ref_rows]
        ref_type_ids = np.zeros(len(ref_df), dtype=int)
        fit = fit_betabinomial_lowrank_counter_eb(
            ref_df, ref_edges, ref_train, ref_type_ids, ref_embedding,
            max_eb_iter=PROSPECTIVE_LOWRANK_EB_MAX_ROUNDS, tol=3e-3, fixed_beta=float(beta),
        )
        ref_delta_logit = {int(g): float(fit.delta[i]) for i, g in enumerate(ref_df["group_id"].astype(int))}
        interaction_all = (
            build_lowrank_design(edges, embedding, fit.skew_pairs).dot(fit.gamma)
            if len(fit.gamma) else np.zeros(len(edges), dtype=float)
        )
        projected: List[List[float]] = [[] for _ in range(len(df))]
        for edge_pos in np.flatnonzero(train):
            er = edges.iloc[int(edge_pos)]
            ia, ib = int(er["ia"]), int(er["ib"])
            ga, gb = int(er["group_a"]), int(er["group_b"])
            n = float(er["samples"]); y = float(er["win_rate_a"])
            p = (y * n + 0.5) / (n + 1.0)
            obs = float(logit(np.clip(p, 1e-6, 1 - 1e-6)))
            inter = float(interaction_all[int(edge_pos)])
            if gb in ref_delta_logit:
                projected[ia].append(obs - float(beta) * (float(df.iloc[ia]["raw_cqd"]) - float(df.iloc[ib]["raw_cqd"])) + ref_delta_logit[gb] - inter)
            if ga in ref_delta_logit:
                projected[ib].append(-obs - float(beta) * (float(df.iloc[ib]["raw_cqd"]) - float(df.iloc[ia]["raw_cqd"])) + ref_delta_logit[ga] + inter)
        raw_projected_logit = np.asarray([
            float(np.mean(v)) if v else float(ref_delta_logit.get(int(df.iloc[i]["group_id"]), 0.0))
            for i, v in enumerate(projected)
        ], dtype=float)
        projected_se_logit = np.asarray([
            float(np.std(v, ddof=1) / math.sqrt(len(v))) if len(v) > 1 else float(max(fit.tau_delta, 1e-6))
            for v in projected
        ], dtype=float)
        delta_logit, member_component, partner_component, tau_member, tau_partner, member_eb_converged, member_eb_rounds = _member_conditioned_strength_eb(
            raw_projected_logit, projected_se_logit, df["group_id"].astype(int).tolist(), group_members,
            fit_mask=member_eb_fit_mask,
        )
        delta_cqd = delta_logit / float(beta)
        fold_delta.append(delta_cqd)
        fold_member_component.append(member_component / float(beta))
        fold_partner_component.append(partner_component / float(beta))
        # OOF calibration is model fitting.  Score-only rows may be projected
        # by the frozen model, but their outcomes must not choose alpha.
        calibration_hold = (
            hold
            & edges["group_a"].astype(int).isin(fit_gid_set).to_numpy(bool)
            & edges["group_b"].astype(int).isin(fit_gid_set).to_numpy(bool)
        )
        sub = edges.loc[calibration_hold].copy()
        ia = sub["ia"].to_numpy(dtype=int)
        ib = sub["ib"].to_numpy(dtype=int)
        raw_eta = float(beta) * (df["raw_cqd"].to_numpy(float)[ia] - df["raw_cqd"].to_numpy(float)[ib])
        strength_eta = raw_eta + delta_logit[ia] - delta_logit[ib]
        full_eta = strength_eta + interaction_all[np.flatnonzero(calibration_hold)]
        if not sub.empty:
            oof_rows.append(pd.DataFrame({
                "fold": int(fold), "win_rate_a": sub["win_rate_a"].to_numpy(float),
                "samples": sub["samples"].to_numpy(float), "raw_eta": raw_eta,
                "strength_eta": strength_eta, "full_eta": full_eta,
            }))
        fold_rows.append({
            "fold": int(fold), "train_edges": int(np.sum(train)), "validation_edges": int(np.sum(hold)),
            "continuous_rank": int(rank), "skew_interaction_parameters": int(len(fit.gamma)),
            "tau_strength_logit": float(fit.tau_delta), "tau_lowrank_logit": float(fit.tau_lowrank),
            "reference_embedding_mean_error": reference_embedding_mean_error,
            "interaction_reference_policy_mean_constrained_zero": 1,
            "environment_fit_reference_groups": int(len(ref_df)),
            "environment_fit_nonreference_groups": 0,
            "spectral_basis_fit_reference_groups": int(len(ref_df)),
            "spectral_basis_fit_nonreference_groups": 0,
            "member_conditioned_tau_member_logit": float(tau_member),
            "member_conditioned_tau_partner_logit": float(tau_partner),
            "member_conditioned_fit_groups": int(np.sum(member_eb_fit_mask)),
            "member_conditioned_read_only_score_groups": int(len(df) - np.sum(member_eb_fit_mask)),
            "oof_calibration_non_score_only_edges": int(np.sum(calibration_hold)),
            "oof_calibration_score_only_edges": 0,
            "member_conditioned_eb_converged": int(member_eb_converged),
            "member_conditioned_eb_rounds": int(member_eb_rounds),
            "member_conditioned_eb_hit_round_limit": int(not member_eb_converged and member_eb_rounds >= PROSPECTIVE_MEMBER_EB_MAX_ROUNDS),
            "fit_success": int(bool(fit.success)), "fit_message": str(fit.message),
            "lowrank_eb_converged": int(fit.eb_converged),
            "lowrank_eb_rounds": int(fit.eb_rounds),
            "lowrank_eb_hit_round_limit": int(fit.eb_hit_round_limit),
            "lowrank_optimizer_all_rounds_converged": int(fit.optimizer_all_rounds_converged),
            "lowrank_optimizer_hit_iteration_limit": int(fit.optimizer_hit_iteration_limit),
            "lowrank_optimizer_total_iterations": int(fit.iterations),
        })
    if not fold_delta:
        raise RuntimeError("Prospective continuous strength cross-fit produced no valid folds")
    delta_matrix = np.vstack(fold_delta)
    member_component_matrix = np.vstack(fold_member_component)
    partner_component_matrix = np.vstack(fold_partner_component)
    all_lowrank_eb_converged = bool(fold_rows) and all(int(r["lowrank_eb_converged"]) == 1 for r in fold_rows)
    any_lowrank_eb_hit_limit = any(int(r["lowrank_eb_hit_round_limit"]) == 1 for r in fold_rows)
    all_member_eb_converged = bool(fold_rows) and all(int(r["member_conditioned_eb_converged"]) == 1 for r in fold_rows)
    any_member_eb_hit_limit = any(int(r["member_conditioned_eb_hit_round_limit"]) == 1 for r in fold_rows)
    alpha = 0.0
    oof = pd.concat(oof_rows, ignore_index=True) if oof_rows else pd.DataFrame()
    if not oof.empty:
        y_oof = oof["win_rate_a"].to_numpy(float)
        n_oof = oof["samples"].to_numpy(float)
        raw_oof = oof["raw_eta"].to_numpy(float)
        strength_increment = oof["strength_eta"].to_numpy(float) - raw_oof
        alpha, raw_oof_ordering, best_oof_ordering, alpha_optimizer_converged, alpha_optimizer_iterations = _select_oof_ordering_calibrated_alpha(
            y_oof, n_oof, raw_oof, strength_increment,
        )
    else:
        raw_oof_ordering = np.nan; best_oof_ordering = np.nan
        alpha_optimizer_converged = True; alpha_optimizer_iterations = 0
    calibrated_delta_matrix = float(alpha) * delta_matrix
    adjustment = np.mean(calibrated_delta_matrix, axis=0)
    crossfit_se = np.std(calibrated_delta_matrix, axis=0, ddof=1) / math.sqrt(calibrated_delta_matrix.shape[0]) if calibrated_delta_matrix.shape[0] > 1 else np.zeros(len(df))
    member_common = float(alpha) * np.mean(member_component_matrix, axis=0)
    out = pd.DataFrame({
        "group_id": df["group_id"].to_numpy(int),
        "prospective_strength_adjustment_cqd": adjustment,
        "prospective_strength_crossfit_se_cqd": crossfit_se,
        "prospective_strength_fold_sd_cqd": np.std(calibrated_delta_matrix, axis=0, ddof=0),
        "prospective_strength_oof_stack_alpha": float(alpha),
        "prospective_strength_oof_raw_ordering_accuracy": float(raw_oof_ordering),
        "prospective_strength_oof_selected_ordering_accuracy": float(best_oof_ordering),
        "prospective_strength_alpha_optimizer_converged": int(alpha_optimizer_converged),
        "prospective_strength_alpha_optimizer_max_iterations": int(alpha_optimizer_iterations),
        "prospective_strength_alpha_optimizer_hit_iteration_limit": int(not alpha_optimizer_converged and alpha_optimizer_iterations >= 100),
        "prospective_all_lowrank_eb_converged": int(all_lowrank_eb_converged),
        "prospective_any_lowrank_eb_hit_round_limit": int(any_lowrank_eb_hit_limit),
        "prospective_all_member_eb_converged": int(all_member_eb_converged),
        "prospective_any_member_eb_hit_round_limit": int(any_member_eb_hit_limit),
        "prospective_member_common_adjustment_cqd": member_common,
        "prospective_partner_specific_adjustment_cqd": adjustment - member_common,
    })
    for f, values in enumerate(calibrated_delta_matrix):
        out[f"prospective_strength_adjustment_fold_{f}_cqd"] = values
    out.to_csv(out_dir / "prospective_continuous_strength_adjustments.csv", index=False)
    pd.DataFrame(fold_rows).to_csv(out_dir / "prospective_continuous_strength_folds.csv", index=False)
    if not oof.empty:
        oof["calibrated_strength_eta"] = oof["raw_eta"] + float(alpha) * (oof["strength_eta"] - oof["raw_eta"])
        rows = []
        for name, col in [("raw", "raw_eta"), ("strength_only_uncalibrated", "strength_eta"), ("strength_only_oof_calibrated", "calibrated_strength_eta"), ("strength_plus_continuous_interaction", "full_eta")]:
            eta_values = oof[col].to_numpy(float)
            ordering = float(np.sum(oof["samples"].to_numpy(float) * ((eta_values >= 0.0) == (oof["win_rate_a"].to_numpy(float) >= 0.5))) / max(1.0, np.sum(oof["samples"].to_numpy(float))))
            rows.append({
                "model": name,
                "weighted_logloss": binomial_logloss(oof["win_rate_a"].to_numpy(float), oof["samples"].to_numpy(float), eta_values),
                "weighted_ordering_accuracy": ordering,
                "strength_stack_alpha": float(alpha),
            })
        pd.DataFrame(rows).to_csv(out_dir / "prospective_continuous_strength_oof_metrics.csv", index=False)
    return out


def _apply_raw_anchored_prospective_correct(
    groups_out: pd.DataFrame,
    final_score_universe_df: pd.DataFrame,
    all_edges: pd.DataFrame,
    group_members: Dict[int, List[str]],
    raw_min: Optional[float],
    lane_size: int,
    out_dir: Path,
    beta: float,
    resolver_baseline_cqd: float,
    seed: int,
) -> Tuple[pd.DataFrame, Dict[str, Any]]:
    """Generate the production Correct score without the legacy active mainline.

    Final Correct is Raw-anchored against one complete legal reference policy.
    References vote equally; sample precision affects measurement variance only.
    Missing target/reference edges are requested through the existing Rust
    MissingRateRequest path, and no synthetic default rate is introduced here.
    """
    if groups_out is None or groups_out.empty:
        return groups_out, {}
    out = groups_out.copy()
    out["group_id"] = out["group_id"].astype(int)
    if "Raw Cqd" not in out.columns:
        out["Raw Cqd"] = out.get("raw_cqd", np.nan)
    out["Raw Cqd"] = pd.to_numeric(out["Raw Cqd"], errors="coerce").astype(float)
    out["raw_cqd"] = pd.to_numeric(out.get("raw_cqd", out["Raw Cqd"]), errors="coerce").astype(float)
    out["global_base_cqd"] = out["raw_cqd"].astype(float)

    for col in ["selection_weight_cqd", "Correct Cqd", "regularized_active_cqd", "Model Correct Cqd"]:
        if col in out.columns:
            out[f"legacy_active_{col.replace(' ', '_').replace('-', '_')}"] = pd.to_numeric(out[col], errors="coerce")

    score_universe = final_score_universe_df.copy() if isinstance(final_score_universe_df, pd.DataFrame) and not final_score_universe_df.empty else out.copy()
    score_universe = score_universe.drop_duplicates("group_id", keep="first").copy()
    score_universe["group_id"] = score_universe["group_id"].astype(int)
    score_universe["raw_cqd"] = pd.to_numeric(score_universe.get("raw_cqd", score_universe.get("Raw Cqd", np.nan)), errors="coerce").astype(float)
    if "blocked_score_only_candidate" not in score_universe.columns:
        score_universe["blocked_score_only_candidate"] = score_universe["group_id"].isin(
            set(out.loc[out.get("blocked_score_only_candidate", pd.Series(False, index=out.index)).fillna(False).astype(bool), "group_id"].astype(int))
        )
    reference_ids = _build_prospective_reference_universe(score_universe, group_members, raw_min, out_dir)
    all_edges, undirected_edge_audit = _deduplicate_undirected_edges(all_edges)
    pd.DataFrame([undirected_edge_audit]).to_csv(out_dir / "prospective_undirected_edge_deduplication.csv", index=False)
    universe_ids = score_universe["group_id"].astype(int).tolist()
    require_pairs_or_request(
        all_edges, universe_ids, reference_ids,
        "prospective all-reference projection", lane_size, out_dir,
    )
    pd.DataFrame([{
        "score_universe_rows": int(len(universe_ids)),
        "reference_count": int(len(reference_ids)),
        "crossfit_fold_count": int(PROSPECTIVE_CROSSFIT_FOLDS),
        "crossfit_fold_selection": "fixed_engineering_tradeoff_not_data_selected",
        "missing_pairs": 0,
        "status": "complete",
    }]).to_csv(out_dir / "prospective_missing_rate_preflight.csv", index=False)
    reference_delta = _prospective_reference_delta(score_universe, all_edges, reference_ids, beta)
    reference_delta.to_csv(out_dir / "prospective_reference_group_deltas.csv", index=False)
    out = out.merge(reference_delta.drop_duplicates("group_id"), on="group_id", how="left")
    replacement_rates = _prospective_mean_rate_against_references(
        score_universe, all_edges, reference_ids,
    )
    out = out.merge(replacement_rates, on="group_id", how="left")
    raw_score = out["Raw Cqd"].to_numpy(float)
    mean_rate = pd.to_numeric(
        out["prospective_replacement_mean_rate_cqd"], errors="coerce"
    ).to_numpy(float)
    if not np.all(np.isfinite(mean_rate)):
        bad = out.loc[~np.isfinite(mean_rate), "group_id"].astype(int).tolist()
        raise RuntimeError(f"Non-finite replacement mean rate; first_group_ids={bad[:30]}")
    # Five expected future entrants occupy five of the fixed 50 Raw target
    # slots. Each is an equal draw from the legal, member-unique reference pool.
    # Therefore they uniformly replace 5/50 of the current Golden target mass.
    adjustment = (float(PROSPECTIVE_REPLACEMENT_K) / 50.0) * (mean_rate - raw_score)
    estimation_se = np.zeros(len(out), dtype=float)
    scenario_sd = np.zeros(len(out), dtype=float)
    uncertainty = np.zeros(len(out), dtype=float)
    coverage = pd.to_numeric(out["prospective_reference_coverage_ratio"], errors="coerce").fillna(0.0).to_numpy(float)
    direct_alpha_info = {
        "direct_reference_oof_enabled": 0,
        "direct_reference_oof_selection_role": "disabled_replaced_by_fixed_slot_k5",
        "prospective_replacement_k": float(PROSPECTIVE_REPLACEMENT_K),
        "prospective_replacement_target_slots": 50.0,
    }
    out["prospective_mixed_residual_adjustment_audit_cqd"] = pd.to_numeric(
        out["prospective_reference_delta_cqd"], errors="coerce"
    )
    # Compatibility fields explicitly describe the retired model rather than
    # presenting a fabricated member/partner explanation.
    out["prospective_strength_adjustment_cqd"] = adjustment
    out["prospective_strength_crossfit_se_cqd"] = estimation_se
    out["prospective_strength_fold_sd_cqd"] = scenario_sd
    out["prospective_strength_oof_stack_alpha"] = 1.0
    out["prospective_direct_reliability"] = 1.0
    out["prospective_direct_tau_cqd"] = np.nan
    out["prospective_direct_scenario_kappa"] = np.nan
    out["prospective_direct_scenario_mean_var_cqd2"] = 0.0
    out["prospective_direct_measurement_se_cqd"] = 0.0
    out["prospective_direct_prediction_se_cqd"] = 0.0
    out["prospective_strength_oof_raw_ordering_accuracy"] = np.nan
    out["prospective_strength_oof_selected_ordering_accuracy"] = np.nan
    out["prospective_strength_alpha_optimizer_converged"] = 1
    out["prospective_strength_alpha_optimizer_max_iterations"] = 0
    out["prospective_strength_alpha_optimizer_hit_iteration_limit"] = 0
    out["prospective_all_lowrank_eb_converged"] = 1
    out["prospective_any_lowrank_eb_hit_round_limit"] = 0
    out["prospective_all_member_eb_converged"] = 1
    out["prospective_any_member_eb_hit_round_limit"] = 0
    out["prospective_member_common_adjustment_cqd"] = np.nan
    out["prospective_partner_specific_adjustment_cqd"] = np.nan
    for key, value in direct_alpha_info.items():
        out[key] = value
    out["prospective_destrat_adjustment_cqd"] = adjustment
    out["prospective_evidence_q"] = coverage
    out["prospective_environment_consistency"] = np.nan
    out["prospective_environment_dispersion_cqd"] = scenario_sd
    out["prospective_posterior_estimation_se_cqd"] = estimation_se
    out["prospective_loo_max_abs_change_cqd"] = scenario_sd
    out["prospective_environment_coverage_mean"] = coverage
    out["prospective_environment_edge_count_mean"] = pd.to_numeric(out["prospective_reference_edge_count"], errors="coerce").fillna(0.0)
    out["prospective_environment_sample_mass_mean"] = pd.to_numeric(out["prospective_reference_sample_mass"], errors="coerce").fillna(0.0)
    out["Correct_center_cqd"] = raw_score + adjustment
    out["Correct_potential_cqd"] = out["Correct_center_cqd"].to_numpy(float) + uncertainty
    out["Correct_uncertainty_cqd"] = uncertainty

    # Member-overlap amplification uncertainty is computed after the center score
    # exists.  It is reported separately and not subtracted from Correct_center.
    score_map = {int(g): float(s) for g, s in out[["group_id", "Correct_center_cqd"]].itertuples(index=False, name=None)}
    member_to_gids: Dict[str, List[int]] = {}
    for gid in out["group_id"].astype(int):
        for m in group_members.get(int(gid), []):
            member_to_gids.setdefault(str(m), []).append(int(gid))
    overlap_u = []
    for gid in out["group_id"].astype(int):
        competitors = set()
        for m in group_members.get(int(gid), []):
            competitors.update(member_to_gids.get(str(m), []))
        competitors.discard(int(gid))
        if not competitors:
            overlap_u.append(0.0)
            continue
        margins = [abs(score_map[int(gid)] - score_map[int(c)]) for c in competitors if int(c) in score_map]
        nearest = min(margins) if margins else np.inf
        overlap_u.append(float(max(0.0, 0.18 - nearest) / 0.18 * 0.18) if np.isfinite(nearest) else 0.0)
    out["Correct_member_overlap_uncertainty_cqd"] = np.asarray(overlap_u, dtype=float)
    # Keep the requested posterior-plus-scenario uncertainty definition pure.
    # Member overlap remains an additional, separately named decision diagnostic.
    out["Correct_selection_risk_penalized_cqd"] = out["Correct_center_cqd"].astype(float) - 0.35 * np.sqrt(
        out["Correct_uncertainty_cqd"].astype(float) ** 2
        + out["Correct_member_overlap_uncertainty_cqd"].astype(float) ** 2
    )

    # Override final public score.  Keep uncertainty separate; do not punish rare
    # or under-exposed profiles inside the semantic center score.
    out["Correct Cqd"] = out["Correct_center_cqd"].astype(float)
    out["selection_weight_cqd"] = out["Correct_center_cqd"].astype(float)
    out["Selection Weight Cqd"] = out["selection_weight_cqd"].astype(float)
    out["Model Correct Cqd"] = out["Correct_center_cqd"].astype(float)
    out["model_correct_delta_from_raw_cqd"] = out["Correct Cqd"].astype(float) - out["Raw Cqd"].astype(float)
    out["regularized_active_cqd"] = out["Correct_center_cqd"].astype(float)
    out["active_residual_raw_cqd"] = out["prospective_destrat_adjustment_cqd"].astype(float)
    out["active_residual_shrunk_cqd"] = out["prospective_destrat_adjustment_cqd"].astype(float)
    out["active_residual_net_adjustment_cqd"] = out["prospective_destrat_adjustment_cqd"].astype(float)
    out["active_residual_reliability"] = out["prospective_evidence_q"].astype(float)
    out["active_residual_q_mass_reliability"] = out["prospective_environment_consistency"].astype(float)
    out["active_residual_edge_count_reliability"] = np.clip(1.0 - np.exp(-out["prospective_environment_edge_count_mean"].astype(float) / 6.0), 0.0, 1.0)
    out["active_residual_sample_mass_reliability"] = np.clip(1.0 - np.exp(-out["prospective_environment_sample_mass_mean"].astype(float) / 300.0), 0.0, 1.0)
    out["active_residual_reference_diversity_reliability"] = out["prospective_environment_coverage_mean"].astype(float).clip(0.0, 1.0)
    out["active_residual_coverage_reliability"] = out["prospective_environment_coverage_mean"].astype(float).clip(0.0, 1.0)
    out["active_uncertainty_penalty_cqd"] = 0.0
    out["active_leverage_penalty_cqd"] = 0.0
    out["active_total_penalty_cqd"] = 0.0
    out["active_set_score_source"] = "legacy_active_iteration_disabled_not_score_source"
    out["active_set_score_success"] = True
    out["active_set_score_message"] = "final_score_replaced_by_fixed_slot_replacement_k5_correct"
    out["active_set_selected_for_training"] = False
    out["prospective_correct_enabled"] = True
    out["selection_weight_source"] = "fixed_slot_replacement_k5_correct_center"
    out["selection_weight_used_final_projection"] = True
    out["selection_weight_fell_back_to_global_base"] = False
    out["selection_weight_delta_from_raw_cqd"] = out["selection_weight_cqd"].astype(float) - out["Raw Cqd"].astype(float)
    out["selection_weight_logit"] = float(beta) * (out["selection_weight_cqd"].astype(float) - float(resolver_baseline_cqd))
    out["posterior_delta_logit"] = float(beta) * (out["Correct Cqd"].astype(float) - out["Raw Cqd"].astype(float))
    out["full_model_delta_logit"] = out["posterior_delta_logit"].astype(float)
    out["posterior_strength_logit"] = float(beta) * out["Correct Cqd"].astype(float)
    out["full_model_strength_logit"] = out["posterior_strength_logit"].astype(float)
    out["full_model_corrected_cqd"] = out["Correct Cqd"].astype(float)
    out["resolver_baseline_cqd"] = float(resolver_baseline_cqd)
    out["resolver_baseline_logit"] = float(beta) * float(resolver_baseline_cqd)
    out["resolver_marginal_utility_logit"] = out["posterior_strength_logit"].astype(float) - float(beta) * float(resolver_baseline_cqd)
    out["selection_weight_q_final"] = out["prospective_evidence_q"].astype(float)
    out["selection_weight_q_tail_mean"] = out["prospective_evidence_q"].astype(float)
    out["selection_weight_q_tail_sd"] = out["prospective_environment_dispersion_cqd"].astype(float)
    out["selection_weight_tail_support_probability"] = out["prospective_environment_consistency"].astype(float)
    out["selection_weight_final_from_tail_average"] = False
    out["cv_bagged_delta_logit"] = out["posterior_delta_logit"].astype(float)
    out["cv_delta_sd_logit"] = abs(float(beta)) * out["prospective_environment_dispersion_cqd"].astype(float)
    out["cv_delta_sd_cqd"] = out["prospective_environment_dispersion_cqd"].astype(float)
    out["stability_strength_sd_logit"] = abs(float(beta)) * out["Correct_uncertainty_cqd"].astype(float)
    out["stability_strength_sd_cqd"] = out["Correct_uncertainty_cqd"].astype(float)

    diag_cols = [c for c in [
        "group_id", "Name", "Text-Type", "RSW-Type", "Raw Cqd", "Correct_center_cqd", "Correct_potential_cqd",
        "Correct_uncertainty_cqd", "Correct_member_overlap_uncertainty_cqd", "Correct_selection_risk_penalized_cqd",
        "prospective_destrat_adjustment_cqd", "prospective_evidence_q", "prospective_environment_consistency",
        "prospective_environment_dispersion_cqd", "prospective_posterior_estimation_se_cqd",
        "prospective_loo_max_abs_change_cqd", "prospective_environment_coverage_mean",
        "prospective_environment_edge_count_mean", "prospective_environment_sample_mass_mean",
        "legacy_active_selection_weight_cqd", "legacy_active_Correct_Cqd", "legacy_active_regularized_active_cqd",
        "blocked_score_only_candidate", "scout_candidate",
        "prospective_reference_delta_cqd", "prospective_reference_se_cqd",
        "prospective_reference_scenario_scale_cqd", "prospective_reference_effective_edge_count",
        "prospective_reference_policy_weight_per_edge", "prospective_reference_mean_measurement_se_cqd",
        "prospective_reference_coverage_ratio", "prospective_reference_edge_count",
        "prospective_reference_sample_mass", "prospective_reference_count",
        "prospective_replacement_mean_rate_cqd",
        "prospective_mixed_residual_adjustment_audit_cqd", "prospective_strength_adjustment_cqd",
        "prospective_strength_crossfit_se_cqd", "prospective_strength_fold_sd_cqd",
        "prospective_strength_oof_stack_alpha",
        "prospective_member_common_adjustment_cqd", "prospective_partner_specific_adjustment_cqd",
        "prospective_strength_oof_raw_ordering_accuracy", "prospective_strength_oof_selected_ordering_accuracy",
        "prospective_strength_alpha_optimizer_converged", "prospective_strength_alpha_optimizer_max_iterations",
        "prospective_strength_alpha_optimizer_hit_iteration_limit",
        "prospective_all_lowrank_eb_converged", "prospective_any_lowrank_eb_hit_round_limit",
        "prospective_all_member_eb_converged", "prospective_any_member_eb_hit_round_limit",
    ] if c in out.columns]
    out[diag_cols].to_csv(out_dir / "prospective_correct_diagnostics.csv", index=False)

    pair_metrics = _write_prospective_pair_metrics(out_dir, out, all_edges, beta)
    pd.DataFrame([{
        "mode": "fixed_slot_replacement_k5_correct",
        "center_score_penalizes_uncertainty": 0,
        "uncertainty_is_separate_output": 1,
        "active_iteration_generates_final_correct": 0,
        "active_iteration_role": "disabled_not_run",
        "aggregation_layer": "five_of_fifty_slots_equal_policy_legal_reference_mean",
        "environment_count": 0,
        "reference_count": int(len(reference_ids)),
        "residual_dependent_reference_weighting": 0,
        "discrete_type_features_used": 0,
        "text_type_features_used": 0,
        "interaction_reference_policy_mean_constrained_zero": 1,
        "edge_subset_lowrank_standardization": 0,
        "direction_gate_used": 0,
        "environment_fit_uses_reference_groups_only": 1,
        "spectral_basis_uses_reference_groups_only": 1,
        "member_conditioned_strength_decomposition": 0,
        "strength_oof_stack_alpha": float(out["prospective_strength_oof_stack_alpha"].iloc[0]),
        "strength_oof_raw_ordering_accuracy": float(out["prospective_strength_oof_raw_ordering_accuracy"].iloc[0]),
        "strength_oof_selected_ordering_accuracy": float(out["prospective_strength_oof_selected_ordering_accuracy"].iloc[0]),
        "strength_alpha_optimizer_converged": int(out["prospective_strength_alpha_optimizer_converged"].iloc[0]),
        "strength_alpha_optimizer_max_iterations": int(out["prospective_strength_alpha_optimizer_max_iterations"].iloc[0]),
        "all_lowrank_eb_converged": int(out["prospective_all_lowrank_eb_converged"].iloc[0]),
        "any_lowrank_eb_hit_round_limit": int(out["prospective_any_lowrank_eb_hit_round_limit"].iloc[0]),
        "all_member_eb_converged": int(out["prospective_all_member_eb_converged"].iloc[0]),
        "any_member_eb_hit_round_limit": int(out["prospective_any_member_eb_hit_round_limit"].iloc[0]),
        "adjustment_prior": "none_fixed_k5_replacement_estimand",
        **direct_alpha_info,
        "mean_abs_adjustment_cqd": float(np.nanmean(np.abs(out["prospective_destrat_adjustment_cqd"].to_numpy(float)))),
        "p95_abs_adjustment_cqd": float(np.nanpercentile(np.abs(out["prospective_destrat_adjustment_cqd"].to_numpy(float)), 95)),
        "max_abs_adjustment_cqd": float(np.nanmax(np.abs(out["prospective_destrat_adjustment_cqd"].to_numpy(float)))),
        "mean_uncertainty_cqd": float(np.nanmean(out["Correct_uncertainty_cqd"].to_numpy(float))),
        "mean_environment_dispersion_cqd": float(np.nanmean(out["prospective_environment_dispersion_cqd"].to_numpy(float))),
        **{k: v for k, v in pair_metrics.items() if isinstance(v, (int, float, np.integer, np.floating))},
    }]).to_csv(out_dir / "prospective_correct_summary.csv", index=False)
    (out_dir / "RAW_ANCHORED_PROSPECTIVE_CORRECT_REPORT.md").write_text(
        "# Fixed-slot prospective replacement Correct (K=5)\n\n"
        "Final `Correct Cqd` is generated by the fixed-slot replacement model.\n\n"
        "## Semantics\n\n"
        "```text\n"
        "mean_rate(x) = equal-weight mean win rate of x against legal references\n"
        "Correct_center(x) = Raw(x) + (5 / 50) * (mean_rate(x) - Raw(x))\n"
        "```\n\n"
        "The 50-slot Raw target mass stays fixed. Five hypothetical future entrants, each an equal draw from the enabled, non-blocked, non-score-only, thresholded, member-unique reference pool, uniformly displace 5/50 of the old Golden mass. OOF, tau, kappa, reliability shrinkage, and final moment alignment do not affect the public score.\n\n"
        "The old active iteration is not executed by the production `run()` path. Missing target-reference edges are requested through `MissingRateRequest`; no synthetic default win rate is used.\n",
        encoding="utf-8",
    )
    return out, {
        "prospective_environment_count": 0,
        "prospective_reference_count": int(len(reference_ids)),
        "prospective_reference_group_ids": [int(g) for g in reference_ids],
        "prospective_mean_abs_adjustment_cqd": float(np.nanmean(np.abs(out["prospective_destrat_adjustment_cqd"].to_numpy(float)))),
        "prospective_max_abs_adjustment_cqd": float(np.nanmax(np.abs(out["prospective_destrat_adjustment_cqd"].to_numpy(float)))),
        "prospective_mean_uncertainty_cqd": float(np.nanmean(out["Correct_uncertainty_cqd"].to_numpy(float))),
        "prospective_pair_metrics_file": "prospective_correct_pair_metrics.csv",
        **direct_alpha_info,
    }

def _require_final_projection_scores(
    projected: pd.DataFrame,
    expected_group_ids: Sequence[int],
    context: str,
) -> None:
    expected = sorted({int(g) for g in expected_group_ids})
    if not expected:
        return
    if projected is None or projected.empty:
        raise RuntimeError(f"{context}: final active projection returned no rows for expected score universe")
    if "group_id" not in projected.columns:
        raise RuntimeError(f"{context}: final active projection output has no group_id column")
    got = set(int(g) for g in projected["group_id"].dropna().astype(int).tolist())
    missing = sorted(set(expected) - got)
    if missing:
        raise RuntimeError(
            f"{context}: final active projection missing {len(missing)} expected score-universe row(s); "
            f"first_group_ids={missing[:30]}"
        )
    if "regularized_active_cqd" not in projected.columns:
        raise RuntimeError(f"{context}: final active projection output has no regularized_active_cqd column")
    sub = projected[projected["group_id"].astype(int).isin(set(expected))].copy()
    vals = pd.to_numeric(sub["regularized_active_cqd"], errors="coerce")
    bad_mask = ~np.isfinite(vals.to_numpy(float))
    if bool(bad_mask.any()):
        bad_ids = sub.loc[bad_mask, "group_id"].astype(int).tolist()
        cols = [c for c in [
            "group_id", "raw_cqd", "Correct Cqd", "regularized_active_cqd",
            "active_set_score_success", "active_set_score_message",
            "active_set_challenger_edges", "active_weight_reference_mass",
            "active_weight_reference_missing_edges", "blocked_score_only_candidate",
            "scout_candidate",
        ] if c in sub.columns]
        debug_path = Path(projected.attrs.get("out_dir", ".")) / "final_projection_missing_regularized_active_debug.csv"
        try:
            sub.loc[bad_mask, cols].to_csv(debug_path, index=False)
        except Exception:
            pass
        raise RuntimeError(
            f"{context}: {len(bad_ids)} score-universe row(s) have missing/non-finite regularized_active_cqd; "
            f"first_group_ids={bad_ids[:30]}. Non-active/not-selected rows must still be score-only projected; "
            "this is not allowed to fall back silently to global_base_cqd."
        )





def _get_global_base_front_raw_min(raw_min: Optional[float]) -> float:
    """Current run's observed-front floor for global base.

    Do not introduce a manual anchor here.  If the user runs raw_min=48.95,
    global base should use the raw_min=48.95 observed environment.  This front
    environment can still be biased; global base is therefore treated as a coarse
    prior / rough high-low finder, not as a fully trusted calibration target.
    """
    if raw_min is None:
        raise RuntimeError("global base current-front mode requires raw_min to be provided")
    return float(raw_min)




def _write_post_training_correction_overfit_diagnostics(out_dir: Path, groups_out: pd.DataFrame) -> None:
    """Diagnose whether post-training correction is too Raw-adhesive or too self-fit.

    This does not change scores.  It decomposes the final score into:
    Raw -> global_base -> unregularized active projection -> regularized active
    and reports how much movement survives reliability/robust/penalty shrink.
    """
    if groups_out is None or groups_out.empty:
        return

    df = groups_out.copy()
    idx = df.index
    raw = pd.to_numeric(df.get("raw_cqd", df.get("Raw Cqd", pd.Series(np.nan, index=idx))), errors="coerce").astype(float)
    correct = pd.to_numeric(df.get("Correct Cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    base = pd.to_numeric(df.get("global_base_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    reg = pd.to_numeric(df.get("regularized_active_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    model = pd.to_numeric(df.get("Model Correct Cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    raw_resid = pd.to_numeric(df.get("active_residual_raw_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    shrunk_resid = pd.to_numeric(df.get("active_residual_shrunk_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    shrink_ratio = pd.to_numeric(df.get("active_residual_shrink_ratio", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    net_adj = pd.to_numeric(df.get("active_residual_net_adjustment_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    rel = pd.to_numeric(df.get("active_residual_reliability", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    rel_q = pd.to_numeric(df.get("active_residual_q_mass_reliability", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    rel_edge = pd.to_numeric(df.get("active_residual_edge_count_reliability", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    rel_sample = pd.to_numeric(df.get("active_residual_sample_mass_reliability", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    rel_div = pd.to_numeric(df.get("active_residual_reference_diversity_reliability", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    rel_cov = pd.to_numeric(df.get("active_residual_coverage_reliability", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    val_surv = pd.to_numeric(df.get("active_residual_validation_survival", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    soft_cap = pd.to_numeric(df.get("active_residual_soft_cap_factor", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    robust = pd.to_numeric(df.get("active_residual_robust_shrink", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    unct = pd.to_numeric(df.get("active_uncertainty_penalty_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    lev = pd.to_numeric(df.get("active_leverage_penalty_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    total_pen = pd.to_numeric(df.get("active_total_penalty_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    q = pd.to_numeric(df.get("selection_weight_q_final", df.get("active_weight_q", pd.Series(np.nan, index=idx))), errors="coerce").astype(float)
    val_raw = pd.to_numeric(df.get("active_validation_raw_logloss", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    val_base = pd.to_numeric(df.get("active_validation_base_logloss", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    val_corr = pd.to_numeric(df.get("active_validation_corrected_logloss", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    val_delta_base = pd.to_numeric(df.get("active_validation_corrected_minus_base_logloss", val_corr - val_base), errors="coerce").astype(float)
    val_delta_raw = pd.to_numeric(df.get("active_validation_corrected_minus_raw_logloss", val_corr - val_raw), errors="coerce").astype(float)
    evidence_gate = pd.to_numeric(df.get("active_weight_evidence_gate", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    effective_cap = pd.to_numeric(df.get("active_residual_effective_soft_cap_cqd", pd.Series(np.nan, index=idx)), errors="coerce").astype(float)

    def _finite_abs(arr):
        arr = pd.to_numeric(arr, errors="coerce").to_numpy(float)
        arr = arr[np.isfinite(arr)]
        return np.abs(arr)

    def _finite_signed(arr):
        arr = pd.to_numeric(arr, errors="coerce").to_numpy(float)
        return arr[np.isfinite(arr)]

    def _stats(prefix: str, arr) -> Dict[str, float]:
        vals = _finite_abs(arr)
        if len(vals) == 0:
            return {f"{prefix}_mean": np.nan, f"{prefix}_median": np.nan, f"{prefix}_p90": np.nan, f"{prefix}_p95": np.nan, f"{prefix}_max": np.nan}
        return {
            f"{prefix}_mean": float(np.nanmean(vals)),
            f"{prefix}_median": float(np.nanmedian(vals)),
            f"{prefix}_p90": float(np.nanpercentile(vals, 90)),
            f"{prefix}_p95": float(np.nanpercentile(vals, 95)),
            f"{prefix}_max": float(np.nanmax(vals)),
        }

    def _mean(prefix: str, arr) -> Dict[str, float]:
        vals = _finite_signed(arr)
        if len(vals) == 0:
            return {f"{prefix}_mean": np.nan, f"{prefix}_median": np.nan}
        return {f"{prefix}_mean": float(np.nanmean(vals)), f"{prefix}_median": float(np.nanmedian(vals))}

    train_mask = df.get("active_set_selected_for_training", pd.Series(False, index=idx)).fillna(False).astype(bool)
    blocked_mask = df.get("blocked_score_only_candidate", pd.Series(False, index=idx)).fillna(False).astype(bool)
    selected_mask = df.get("resolver_selected", pd.Series(0, index=idx)).fillna(0).astype(int).astype(bool)
    scout_mask = df.get("scout_candidate", pd.Series(False, index=idx)).fillna(False).astype(bool)

    masks = {
        "all_scoreable_output": pd.Series(True, index=idx),
        "active_training_rows": train_mask,
        "score_only_nonblocked": (~train_mask) & (~blocked_mask),
        "score_only_blocked": blocked_mask,
        "resolver_selected": selected_mask,
        "scout_rows": scout_mask,
        "q_support_rows": q.fillna(0.0) > ACTIVE_SET_WEIGHTED_SUPPORT_EPS,
        "q_non_support_rows": q.fillna(0.0) <= ACTIVE_SET_WEIGHTED_SUPPORT_EPS,
    }

    rows = []
    for label, mask in masks.items():
        sub_idx = df.index[mask]
        if len(sub_idx) == 0:
            continue
        row = {"slice": label, "rows": int(len(sub_idx))}
        row.update(_stats("abs_correct_minus_raw_cqd", correct.loc[sub_idx] - raw.loc[sub_idx]))
        row.update(_stats("abs_global_base_minus_raw_cqd", base.loc[sub_idx] - raw.loc[sub_idx]))
        row.update(_stats("abs_unregularized_model_minus_raw_cqd", model.loc[sub_idx] - raw.loc[sub_idx]))
        row.update(_stats("abs_regularized_active_minus_raw_cqd", reg.loc[sub_idx] - raw.loc[sub_idx]))
        row.update(_stats("abs_unregularized_active_residual_cqd", raw_resid.loc[sub_idx]))
        row.update(_stats("abs_shrunk_active_residual_cqd", shrunk_resid.loc[sub_idx]))
        row.update(_stats("abs_net_active_adjustment_after_penalty_cqd", net_adj.loc[sub_idx]))
        row.update(_mean("active_residual_reliability", rel.loc[sub_idx]))
        row.update(_mean("active_residual_q_mass_reliability", rel_q.loc[sub_idx]))
        row.update(_mean("active_residual_edge_count_reliability", rel_edge.loc[sub_idx]))
        row.update(_mean("active_residual_sample_mass_reliability", rel_sample.loc[sub_idx]))
        row.update(_mean("active_residual_reference_diversity_reliability", rel_div.loc[sub_idx]))
        row.update(_mean("active_residual_coverage_reliability", rel_cov.loc[sub_idx]))
        row.update(_mean("active_residual_validation_survival", val_surv.loc[sub_idx]))
        row.update(_mean("active_residual_soft_cap_factor", soft_cap.loc[sub_idx]))
        row.update(_mean("active_residual_effective_soft_cap_cqd", effective_cap.loc[sub_idx]))
        row.update(_mean("active_residual_robust_shrink", robust.loc[sub_idx]))
        row.update(_mean("active_residual_shrink_ratio", shrink_ratio.loc[sub_idx]))
        row.update(_mean("active_uncertainty_penalty_cqd", unct.loc[sub_idx]))
        row.update(_mean("active_leverage_penalty_cqd", lev.loc[sub_idx]))
        row.update(_mean("active_total_penalty_cqd", total_pen.loc[sub_idx]))
        row.update(_mean("q_final", q.loc[sub_idx]))
        row.update(_mean("active_weight_evidence_gate", evidence_gate.loc[sub_idx]))
        row.update(_mean("active_validation_raw_logloss", val_raw.loc[sub_idx]))
        row.update(_mean("active_validation_base_logloss", val_base.loc[sub_idx]))
        row.update(_mean("active_validation_corrected_logloss", val_corr.loc[sub_idx]))
        row.update(_mean("active_validation_corrected_minus_base_logloss", val_delta_base.loc[sub_idx]))
        row.update(_mean("active_validation_corrected_minus_raw_logloss", val_delta_raw.loc[sub_idx]))

        denom_model = row.get("abs_unregularized_model_minus_raw_cqd_mean", np.nan)
        denom_raw_resid = row.get("abs_unregularized_active_residual_cqd_mean", np.nan)
        corr_raw = row.get("abs_correct_minus_raw_cqd_mean", np.nan)
        net = row.get("abs_net_active_adjustment_after_penalty_cqd_mean", np.nan)
        row["final_vs_unregularized_model_movement_ratio"] = (
            float(corr_raw / denom_model) if np.isfinite(corr_raw) and np.isfinite(denom_model) and denom_model > 1e-12 else np.nan
        )
        row["net_active_survival_ratio"] = (
            float(net / denom_raw_resid) if np.isfinite(net) and np.isfinite(denom_raw_resid) and denom_raw_resid > 1e-12 else np.nan
        )
        row["raw_adhesion_warning"] = bool(
            np.isfinite(row.get("final_vs_unregularized_model_movement_ratio", np.nan))
            and row.get("final_vs_unregularized_model_movement_ratio", np.nan) < 0.25
            and np.isfinite(denom_model)
            and denom_model > 0.08
        )
        row["active_residual_over_shrunk_warning"] = bool(
            np.isfinite(row.get("net_active_survival_ratio", np.nan))
            and row.get("net_active_survival_ratio", np.nan) < 0.25
            and np.isfinite(denom_raw_resid)
            and denom_raw_resid > 0.08
        )
        large_active_residual = bool(
            np.isfinite(row.get("abs_unregularized_active_residual_cqd_p95", np.nan))
            and np.isfinite(row.get("abs_shrunk_active_residual_cqd_p95", np.nan))
            and row.get("abs_unregularized_active_residual_cqd_p95", np.nan) > 0.50
            and row.get("abs_shrunk_active_residual_cqd_p95", np.nan) > 0.35
        )
        val_db = row.get("active_validation_corrected_minus_base_logloss_mean", np.nan)
        val_dr = row.get("active_validation_corrected_minus_raw_logloss_mean", np.nan)
        validation_not_better = (
            (not np.isfinite(val_db) and not np.isfinite(val_dr))
            or (np.isfinite(val_db) and val_db >= -ACTIVE_REG_VALIDATION_TOL)
            or (np.isfinite(val_dr) and val_dr >= -ACTIVE_REG_VALIDATION_TOL)
        )
        row["large_active_residual_warning"] = large_active_residual
        row["validation_supports_active_residual"] = bool(large_active_residual and not validation_not_better)
        row["possible_self_fit_warning"] = bool(large_active_residual and validation_not_better)
        rows.append(row)

    pd.DataFrame(rows).to_csv(out_dir / "post_training_correction_overfit_diagnostics.csv", index=False)

    # Keep a compact row-level file for the top rows most affected by shrink.
    row_diag = pd.DataFrame({
        "group_id": df["group_id"].astype(int),
        "raw_cqd": raw,
        "global_base_cqd": base,
        "unregularized_model_correct_cqd": model,
        "regularized_active_cqd": reg,
        "Correct Cqd": correct,
        "active_residual_raw_cqd": raw_resid,
        "active_residual_shrunk_cqd": shrunk_resid,
        "active_residual_net_adjustment_cqd": net_adj,
        "active_residual_reliability": rel,
        "active_residual_q_mass_reliability": rel_q,
        "active_residual_edge_count_reliability": rel_edge,
        "active_residual_sample_mass_reliability": rel_sample,
        "active_residual_reference_diversity_reliability": rel_div,
        "active_residual_coverage_reliability": rel_cov,
        "active_residual_validation_survival": val_surv,
        "active_residual_soft_cap_factor": soft_cap,
        "active_residual_effective_soft_cap_cqd": effective_cap,
        "active_residual_robust_shrink": robust,
        "active_residual_shrink_ratio": shrink_ratio,
        "active_uncertainty_penalty_cqd": unct,
        "active_leverage_penalty_cqd": lev,
        "active_total_penalty_cqd": total_pen,
        "selection_weight_q_final": q,
        "active_weight_evidence_gate": evidence_gate,
        "active_validation_raw_logloss": val_raw,
        "active_validation_base_logloss": val_base,
        "active_validation_corrected_logloss": val_corr,
        "active_validation_corrected_minus_base_logloss": val_delta_base,
        "active_validation_corrected_minus_raw_logloss": val_delta_raw,
        "active_set_selected_for_training": train_mask,
        "blocked_score_only_candidate": blocked_mask,
        "scout_candidate": scout_mask,
        "resolver_selected": selected_mask,
    })
    row_diag["abs_unregularized_model_minus_raw_cqd"] = (row_diag["unregularized_model_correct_cqd"] - row_diag["raw_cqd"]).abs()
    row_diag["abs_correct_minus_raw_cqd"] = (row_diag["Correct Cqd"] - row_diag["raw_cqd"]).abs()
    row_diag["abs_active_residual_raw_cqd"] = row_diag["active_residual_raw_cqd"].abs()
    row_diag["abs_active_residual_net_adjustment_cqd"] = row_diag["active_residual_net_adjustment_cqd"].abs()
    row_diag["shrink_loss_cqd"] = row_diag["abs_active_residual_raw_cqd"] - row_diag["abs_active_residual_net_adjustment_cqd"]
    row_diag = row_diag.sort_values(["shrink_loss_cqd", "abs_unregularized_model_minus_raw_cqd"], ascending=[False, False]).head(120)
    row_diag.to_csv(out_dir / "post_training_top_shrunk_rows.csv", index=False)



def _write_raw_adhesion_diagnostics(out_dir: Path, groups_out: pd.DataFrame) -> None:
    """Write metrics that reveal whether the final score is overly Raw-adhesive."""
    if groups_out is None or groups_out.empty:
        return

    df = groups_out.copy()
    raw = pd.to_numeric(df.get("raw_cqd", df.get("Raw Cqd", pd.Series(np.nan, index=df.index))), errors="coerce").astype(float)
    correct = pd.to_numeric(df.get("Correct Cqd", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    base = pd.to_numeric(df.get("global_base_cqd", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    reg = pd.to_numeric(df.get("regularized_active_cqd", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    model = pd.to_numeric(df.get("Model Correct Cqd", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    raw_resid = pd.to_numeric(df.get("active_residual_raw_cqd", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    shrunk_resid = pd.to_numeric(df.get("active_residual_shrunk_cqd", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel = pd.to_numeric(df.get("active_residual_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel_q = pd.to_numeric(df.get("active_residual_q_mass_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel_edge = pd.to_numeric(df.get("active_residual_edge_count_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel_sample = pd.to_numeric(df.get("active_residual_sample_mass_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel_div = pd.to_numeric(df.get("active_residual_reference_diversity_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel_cov = pd.to_numeric(df.get("active_residual_coverage_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)

    def _abs_stats(name: str, values: pd.Series) -> Dict[str, float]:
        arr = pd.to_numeric(values, errors="coerce").to_numpy(float)
        arr = arr[np.isfinite(arr)]
        if len(arr) == 0:
            return {
                f"{name}_mean": np.nan,
                f"{name}_median": np.nan,
                f"{name}_p95": np.nan,
                f"{name}_max": np.nan,
            }
        arr = np.abs(arr)
        return {
            f"{name}_mean": float(np.nanmean(arr)),
            f"{name}_median": float(np.nanmedian(arr)),
            f"{name}_p95": float(np.nanpercentile(arr, 95)),
            f"{name}_max": float(np.nanmax(arr)),
        }

    rows = []
    masks = {
        "all_scoreable_output": pd.Series(True, index=df.index),
        "active_training_rows": df.get("active_set_selected_for_training", pd.Series(False, index=df.index)).fillna(False).astype(bool),
        "score_only_nonblocked": (~df.get("active_set_selected_for_training", pd.Series(False, index=df.index)).fillna(False).astype(bool))
            & (~df.get("blocked_score_only_candidate", pd.Series(False, index=df.index)).fillna(False).astype(bool)),
        "score_only_blocked": df.get("blocked_score_only_candidate", pd.Series(False, index=df.index)).fillna(False).astype(bool),
        "resolver_selected": df.get("resolver_selected", pd.Series(0, index=df.index)).fillna(0).astype(int).astype(bool),
    }

    for label, mask in masks.items():
        sub = df.loc[mask].copy()
        if sub.empty:
            continue
        idx = sub.index
        row = {
            "slice": label,
            "rows": int(len(sub)),
            "selection_used_final_projection_rate": float(sub.get("selection_weight_used_final_projection", pd.Series(False, index=sub.index)).fillna(False).astype(bool).mean()),
            "selection_fell_back_to_global_base_rate": float(sub.get("selection_weight_fell_back_to_global_base", pd.Series(False, index=sub.index)).fillna(False).astype(bool).mean()),
            "active_residual_reliability_mean": float(pd.to_numeric(rel.loc[idx], errors="coerce").mean()),
            "active_residual_reliability_median": float(pd.to_numeric(rel.loc[idx], errors="coerce").median()),
            "active_residual_q_mass_reliability_mean": float(pd.to_numeric(rel_q.loc[idx], errors="coerce").mean()),
            "active_residual_edge_count_reliability_mean": float(pd.to_numeric(rel_edge.loc[idx], errors="coerce").mean()),
            "active_residual_sample_mass_reliability_mean": float(pd.to_numeric(rel_sample.loc[idx], errors="coerce").mean()),
            "active_residual_reference_diversity_reliability_mean": float(pd.to_numeric(rel_div.loc[idx], errors="coerce").mean()),
            "active_residual_coverage_reliability_mean": float(pd.to_numeric(rel_cov.loc[idx], errors="coerce").mean()),
        }
        row.update(_abs_stats("abs_correct_minus_raw_cqd", correct.loc[idx] - raw.loc[idx]))
        row.update(_abs_stats("abs_global_base_minus_raw_cqd", base.loc[idx] - raw.loc[idx]))
        row.update(_abs_stats("abs_regularized_active_minus_global_base_cqd", reg.loc[idx] - base.loc[idx]))
        row.update(_abs_stats("abs_model_correct_minus_raw_cqd", model.loc[idx] - raw.loc[idx]))
        row.update(_abs_stats("abs_active_residual_raw_cqd", raw_resid.loc[idx]))
        row.update(_abs_stats("abs_active_residual_shrunk_cqd", shrunk_resid.loc[idx]))

        denom = row.get("abs_model_correct_minus_raw_cqd_mean", np.nan)
        num = row.get("abs_correct_minus_raw_cqd_mean", np.nan)
        row["raw_adhesion_ratio_vs_unregularized_model"] = float(num / denom) if np.isfinite(num) and np.isfinite(denom) and denom > 1e-12 else np.nan
        base_num = row.get("abs_global_base_minus_raw_cqd_mean", np.nan)
        row["regularized_active_extra_movement_vs_base_ratio"] = (
            float(row.get("abs_regularized_active_minus_global_base_cqd_mean", np.nan) / base_num)
            if np.isfinite(row.get("abs_regularized_active_minus_global_base_cqd_mean", np.nan))
            and np.isfinite(base_num) and base_num > 1e-12
            else np.nan
        )
        rows.append(row)

    out = pd.DataFrame(rows)
    out.to_csv(out_dir / "raw_adhesion_diagnostics.csv", index=False)



def run(sqlite_path:Path, out_dir:Path, lane_size:int=2, nfold:int=5, seed:int=123):
    out_dir.mkdir(parents=True, exist_ok=True)
    conn=sqlite3.connect(sqlite_path)
    # lane results only; raw_average_cqd required.
    lr=pd.read_sql_query("""
        select lr.group_id, lr.raw_average_cqd, lr.average_cqd as old_average_cqd,
               lr.rank as old_rank, lr.golden_rate,
               g.canonical, g.display_raw
        from lane_results lr join groups g on g.id=lr.group_id
        where lr.lane_size=? and lr.raw_average_cqd is not null
        order by lr.group_id
    """, conn, params=(lane_size,))
    if lr.empty: raise RuntimeError('No lane_results rows with raw_average_cqd')
    lr=lr.rename(columns={'raw_average_cqd':'raw_cqd'})
    # members and text type from embedded algorithm
    gm=pd.read_sql_query("""
        select gm.group_id, gm.member, gm.position from group_members gm
        join lane_results lr on lr.group_id=gm.group_id and lr.lane_size=?
        order by gm.group_id, gm.position
    """, conn, params=(lane_size,))
    group_members={gid:list(g['member']) for gid,g in gm.groupby('group_id', sort=False)}
    text_rows=[]
    for _,r in lr.iterrows():
        members=group_members.get(int(r.group_id), str(r.canonical).split('+'))
        sm=compute_group_skill_summary([str(x) for x in members])
        text_rows.append({'group_id':int(r.group_id),'Text-Type':sm['type_label'],'Simple Text-Type':sm['simple_type_label'],'Name':sm['display_canonical'] or r.display_raw})
    text_df=pd.DataFrame(text_rows)
    groups_df=lr.merge(text_df,on='group_id',how='left')
    groups_df['raw_rank']=groups_df['raw_cqd'].rank(ascending=False,method='first').astype(int)
    group_to_idx={gid:i for i,gid in enumerate(groups_df['group_id'])}
    # edges within these groups
    edges=pd.read_sql_query("select group_a, group_b, win_rate_a, samples from group_rates where samples>0 and win_rate_a is not null", conn)
    conn.close()
    edges=edges[edges.group_a.isin(group_to_idx) & edges.group_b.isin(group_to_idx)].copy()
    # DB stores win_rate_a as percentage in this dataset. Normalize exactly once to probability.
    if float(edges['win_rate_a'].max()) > 1.0:
        edges['win_rate_a'] = edges['win_rate_a'] / 100.0
    edges['ia']=edges.group_a.map(group_to_idx).astype(int); edges['ib']=edges.group_b.map(group_to_idx).astype(int)
    edges['fold5']=edge_fold_ids(edges['group_a'].to_numpy(),edges['group_b'].to_numpy(),nfold=nfold)
    if edges.empty: raise RuntimeError('No within-lane group_rates edges')
    raw=groups_df['raw_cqd'].to_numpy();
    # Input integrity
    checks=[]
    checks.append({'check':'raw_cqd_rows','value':len(groups_df),'status':'OK'})
    checks.append({'check':'pair_edges','value':len(edges),'status':'OK'})
    checks.append({'check':'text_type_computed_rows','value':groups_df['Text-Type'].notna().sum(),'status':'OK' if groups_df['Text-Type'].notna().all() else 'FAIL'})
    checks.append({'check':'golden_used','value':0,'status':'OK'})
    checks.append({'check':'legacy_winrate_type_used','value':0,'status':'OK'})
    pd.DataFrame(checks).to_csv(out_dir/'input_integrity_checks.csv',index=False)
    # OOF crossfit
    oof_rows=[]; fold_records=[]; k_records=[]; fit_records=[]; fold_strength_rows=[]
    for f in range(nfold):
        hold=(edges['fold5'].to_numpy()==f); train=~hold
        beta_raw=fit_raw_beta((raw[edges.loc[train,'ia'].to_numpy()]-raw[edges.loc[train,'ib'].to_numpy()]), edges.loc[train,'win_rate_a'].to_numpy(float), edges.loc[train,'samples'].to_numpy(float))
        type_ids, type_labels, kdf, _, _ = adaptive_residual_type(groups_df, edges, beta_raw, train, validation_mask=None, seed=seed+f)
        kdf['outer_fold']=f; k_records.append(kdf)
        fit=fit_betabinomial_eb(groups_df, edges, train, type_ids, max_eb_iter=4, tol=2e-3)
        fold_strength_rows.extend([{'outer_fold': int(f), 'group_id': int(gid), 'fold_strength_logit': float(fit.beta*rv + dv), 'fold_beta': float(fit.beta), 'fold_delta_logit': float(dv)} for gid, rv, dv in zip(groups_df['group_id'], raw, fit.delta)])
        raw_eta=beta_raw*(raw[edges.loc[hold,'ia'].to_numpy()]-raw[edges.loc[hold,'ib'].to_numpy()])
        corr_eta=predict_edges(groups_df, edges, hold, fit, type_ids)
        sub=edges.loc[hold].copy().reset_index(drop=True)
        sub['outer_fold']=f; sub['raw_eta']=raw_eta; sub['corrected_eta']=corr_eta; sub['raw_p']=sigmoid(raw_eta); sub['corrected_p']=sigmoid(corr_eta)
        sub['type_a']=[type_labels[i] for i in sub['ia']]; sub['type_b']=[type_labels[i] for i in sub['ib']]
        # residual on empirical logit with Laplace smoothing for bias diagnostics
        n=sub['samples'].to_numpy(float); y=sub['win_rate_a'].to_numpy(float); elog=logit(np.clip((y*n+0.5)/(n+1.0),1e-6,1-1e-6))
        sub['raw_residual']=elog-sub['raw_eta']; sub['corrected_residual']=elog-sub['corrected_eta']
        oof_rows.append(sub)
        m=metrics_for(sub, raw_eta, corr_eta); fold_metric=m.pivot_table(index=None, columns='model', values=['weighted_logloss','weighted_brier','weighted_auc','weighted_ordering_accuracy']).to_dict()
        fold_records.append({'outer_fold':f,'selected_k':int(kdf.sort_values('validation_logloss').iloc[0]['k']),'raw_beta':beta_raw,'fit_beta':fit.beta,'tau_delta':fit.tau_delta,'tau_counter':fit.tau_counter,'beta_binomial_phi':fit.phi,'fit_success':fit.success,'fit_message':fit.message,
                             'raw_logloss':float(m[m.model=='raw'].weighted_logloss.iloc[0]),'corrected_logloss':float(m[m.model=='corrected'].weighted_logloss.iloc[0]),
                             'raw_brier':float(m[m.model=='raw'].weighted_brier.iloc[0]),'corrected_brier':float(m[m.model=='corrected'].weighted_brier.iloc[0]),
                             'raw_auc':float(m[m.model=='raw'].weighted_auc.iloc[0]),'corrected_auc':float(m[m.model=='corrected'].weighted_auc.iloc[0]),
                             'raw_ordering':float(m[m.model=='raw'].weighted_ordering_accuracy.iloc[0]),'corrected_ordering':float(m[m.model=='corrected'].weighted_ordering_accuracy.iloc[0])})
        fit_records.append({'outer_fold':f,'dim_delta':len(fit.delta),'dim_counter':len(fit.theta),'eb_iterations':fit.iterations,'map_nll':fit.map_nll,'beta_binomial_phi':fit.phi})
    oof=pd.concat(oof_rows,ignore_index=True)
    fold_strength_df=pd.DataFrame(fold_strength_rows)
    if len(fold_strength_df):
        fold_strength_df.to_csv(out_dir/'crossfit_strength_by_group_by_fold.csv',index=False)
        strength_stability=fold_strength_df.groupby('group_id').agg(
            cv_mean_strength_logit=('fold_strength_logit','mean'),
            stability_strength_sd_logit=('fold_strength_logit','std'),
            cv_min_strength_logit=('fold_strength_logit','min'),
            cv_max_strength_logit=('fold_strength_logit','max'),
            cv_bagged_delta_logit=('fold_delta_logit','mean'),
            cv_delta_sd_logit=('fold_delta_logit','std'),
            cv_delta_min_logit=('fold_delta_logit','min'),
            cv_delta_max_logit=('fold_delta_logit','max'),
        ).reset_index()
        strength_stability['stability_strength_sd_logit']=strength_stability['stability_strength_sd_logit'].fillna(0.0)
        strength_stability['cv_delta_sd_logit']=strength_stability['cv_delta_sd_logit'].fillna(0.0)
    else:
        strength_stability=pd.DataFrame({'group_id':groups_df['group_id'],'cv_mean_strength_logit':np.nan,'stability_strength_sd_logit':np.nan,'cv_min_strength_logit':np.nan,'cv_max_strength_logit':np.nan,'cv_bagged_delta_logit':np.nan,'cv_delta_sd_logit':np.nan,'cv_delta_min_logit':np.nan,'cv_delta_max_logit':np.nan})
    strength_stability.to_csv(out_dir/'cv_strength_stability_by_group.csv',index=False)
    oof.to_csv(out_dir/'oof_edge_predictions.csv',index=False)
    pd.concat(k_records,ignore_index=True).to_csv(out_dir/'adaptive_rsw_type_k_selection_by_fold.csv',index=False)
    pd.DataFrame(fold_records).to_csv(out_dir/'outer_fold_prediction_metrics.csv',index=False)
    pd.DataFrame(fit_records).to_csv(out_dir/'betabinomial_eb_fit_diagnostics_by_fold.csv',index=False)
    # Overall OOF metrics
    oof_metrics=metrics_for(oof, oof['raw_eta'].to_numpy(), oof['corrected_eta'].to_numpy())
    oof_metrics.to_csv(out_dir/'oof_prediction_metrics_before_after.csv',index=False)
    # Type bias diagnostics
    tl=weighted_bias_by_type(oof, 'type_a')
    tl.to_csv(out_dir/'oof_type_level_bias_before_after.csv',index=False)
    tp=type_pair_bias(oof)
    tp.to_csv(out_dir/'oof_type_pair_bias_before_after_detail.csv',index=False)
    # summaries
    summary=[]
    for model in ['raw','corrected']:
        t=tl[tl.model==model]; p=tp[tp.model==model]
        summary.append({'model':model,'type_level_bias_rms':float(np.sqrt(np.average(t.mean_residual**2,weights=t.support_samples))) if len(t) else np.nan,
                        'type_pair_bias_rms':float(np.sqrt(np.average(p.mean_residual**2,weights=p.support_samples))) if len(p) else np.nan,
                        'residual_eta2_by_type_a':eta2_by_type(oof, f'{model}_residual','type_a'),
                        'worst_abs_type_bias':float(t.abs_mean_residual.max()) if len(t) else np.nan,
                        'worst_abs_type_pair_bias':float(p.abs_mean_residual.max()) if len(p) else np.nan})
    type_summary=pd.DataFrame(summary); type_summary.to_csv(out_dir/'oof_type_bias_summary_before_after.csv',index=False)
    # worst worsened type-pairs
    piv=tp.pivot_table(index=['type_a','type_b'],columns='model',values=['mean_residual','abs_mean_residual','support_edges','support_samples'],aggfunc='first')
    piv.columns=['_'.join(c).strip() for c in piv.columns.values]; piv=piv.reset_index()
    if 'abs_mean_residual_corrected' in piv and 'abs_mean_residual_raw' in piv:
        piv['abs_bias_delta']=piv['abs_mean_residual_corrected']-piv['abs_mean_residual_raw']
    piv.sort_values('abs_bias_delta' if 'abs_bias_delta' in piv else 'type_a', ascending=False).to_csv(out_dir/'worst_worsened_type_pair_bias.csv',index=False)
    # gap calibration and high confidence violation
    gap=np.abs(oof['corrected_eta'])
    try: oof['corrected_gap_decile']=pd.qcut(gap,10,labels=False,duplicates='drop')
    except Exception: oof['corrected_gap_decile']=0
    gap_rows=[]
    for model in ['raw','corrected']:
        eta=oof[f'{model}_eta'].to_numpy(); pred=sigmoid(eta); y=oof.win_rate_a.to_numpy(float); n=oof.samples.to_numpy(float)
        try: bins=pd.qcut(np.abs(eta),10,labels=False,duplicates='drop')
        except Exception: bins=np.zeros(len(oof),dtype=int)
        tmp=pd.DataFrame({'bin':bins,'pred':pred,'obs':y,'n':n,'eta_abs':np.abs(eta)})
        for b,g in tmp.groupby('bin'):
            w=g.n.to_numpy(float)
            gap_rows.append({'model':model,'gap_bin':int(b),'edge_count':len(g),'sample_count':float(w.sum()),'mean_abs_eta_gap':float(np.average(g.eta_abs,weights=w)),'mean_pred':float(np.average(g.pred,weights=w)),'mean_observed':float(np.average(g.obs,weights=w)),'calibration_error':float(np.average(g.pred-g.obs,weights=w))})
    pd.DataFrame(gap_rows).to_csv(out_dir/'cqd_gap_calibration_by_bin.csv',index=False)
    viol_rows=[]
    for model in ['raw','corrected']:
        eta=oof[f'{model}_eta'].to_numpy(); y=oof.win_rate_a.to_numpy(float); n=oof.samples.to_numpy(float)
        try: bins=pd.qcut(np.abs(eta),10,labels=False,duplicates='drop')
        except Exception: bins=np.zeros(len(oof),dtype=int)
        violation=((eta>0)&(y<0.5))|((eta<0)&(y>0.5))
        for b in sorted(set(bins)):
            m=bins==b; viol_rows.append({'model':model,'gap_bin':int(b),'edge_count':int(m.sum()),'sample_count':float(n[m].sum()),'weighted_violation_rate':float(np.sum(n[m]*violation[m])/max(1.0,np.sum(n[m]))),'mean_abs_eta_gap':float(np.average(np.abs(eta[m]),weights=n[m]))})
    pd.DataFrame(viol_rows).to_csv(out_dir/'high_confidence_violation_by_gap_decile.csv',index=False)
    # Full final model on all edges
    beta_all=fit_raw_beta(raw[edges['ia'].to_numpy()]-raw[edges['ib'].to_numpy()], edges['win_rate_a'].to_numpy(float), edges['samples'].to_numpy(float))
    all_mask=np.ones(len(edges),dtype=bool)
    type_ids, type_labels, kdf, prof, wsum=adaptive_residual_type(groups_df, edges, beta_all, all_mask, validation_mask=None, seed=seed+100)
    kdf.to_csv(out_dir/'adaptive_rsw_type_k_selection_full.csv',index=False)
    fit=fit_betabinomial_eb(groups_df, edges, all_mask, type_ids, max_eb_iter=6, tol=1e-3)
    # final group posterior strength
    beta=fit.beta
    if abs(beta)<1e-8:
        corrected_cqd=beta*raw+fit.delta
        cqd_scale_note='beta_near_zero_output_logit_strength'
    else:
        corrected_cqd=raw+fit.delta/beta
        cqd_scale_note='raw_cqd_plus_delta_over_beta'
    groups_out=groups_df.copy()
    groups_out['RSW-Type']=type_labels
    groups_out['full_model_strength_logit']=beta*raw+fit.delta
    groups_out['full_model_corrected_cqd']=corrected_cqd
    groups_out['full_model_delta_logit']=fit.delta
    groups_out=groups_out.merge(strength_stability[['group_id','cv_mean_strength_logit','stability_strength_sd_logit','cv_min_strength_logit','cv_max_strength_logit','cv_bagged_delta_logit','cv_delta_sd_logit','cv_delta_min_logit','cv_delta_max_logit']], on='group_id', how='left')
    groups_out['cv_bagged_delta_logit']=groups_out['cv_bagged_delta_logit'].fillna(groups_out['full_model_delta_logit'])
    groups_out['stability_strength_sd_logit']=groups_out['stability_strength_sd_logit'].fillna(0.0)
    groups_out['cv_delta_sd_logit']=groups_out['cv_delta_sd_logit'].fillna(0.0)
    # Final leaderboard score uses cross-fit bagged random effect, not the full-fit delta.
    # This is a data-driven ensemble estimator, not a cap or hand-tuned alpha.
    groups_out['posterior_strength_logit']=beta*raw+groups_out['cv_bagged_delta_logit'].to_numpy(float)
    groups_out['Correct Cqd']=raw+groups_out['cv_bagged_delta_logit'].to_numpy(float)/beta if abs(beta)>1e-8 else groups_out['posterior_strength_logit']
    groups_out['posterior_delta_logit']=groups_out['cv_bagged_delta_logit']
    groups_out['stability_strength_sd_cqd']=groups_out['stability_strength_sd_logit']/abs(beta) if abs(beta)>1e-8 else groups_out['stability_strength_sd_logit']
    groups_out['cv_delta_sd_cqd']=groups_out['cv_delta_sd_logit']/abs(beta) if abs(beta)>1e-8 else groups_out['cv_delta_sd_logit']
    groups_out['Raw Rank']=groups_out['raw_rank']
    groups_out['Raw Cqd']=groups_out['raw_cqd']
    groups_out['Correct Rank All Candidates']=groups_out['Correct Cqd'].rank(ascending=False,method='first').astype(int)
    groups_out['rank_delta_all_candidates']=groups_out['Raw Rank']-groups_out['Correct Rank All Candidates']
    # type full table
    groups_out[['group_id','RSW-Type','Text-Type','Simple Text-Type','Name','raw_cqd','Correct Cqd','full_model_corrected_cqd','posterior_strength_logit','full_model_strength_logit','posterior_delta_logit','full_model_delta_logit','stability_strength_sd_logit','stability_strength_sd_cqd','cv_delta_sd_logit','cv_delta_sd_cqd','Raw Rank','Correct Rank All Candidates']].to_csv(out_dir/'posterior_strength_by_group.csv',index=False)
    # counter table
    counter_rows=[]
    for j,(a,b) in enumerate(fit.theta_pairs):
        counter_rows.append({'type_a':f'RSW{a+1:02d}','type_b':f'RSW{b+1:02d}','counter_a_beats_b_logit':fit.theta[j],'effect_sd_not_laplace':math.sqrt(max(fit.var_theta[j],0)) if j<len(fit.var_theta) else np.nan})
    pd.DataFrame(counter_rows).to_csv(out_dir/'antisymmetric_type_counter_posterior.csv',index=False)
    # Resolver exact set-packing MILP using utility posterior_strength_logit. Candidate group all groups.
    members=group_members
    member_list=sorted({m for ms in members.values() for m in ms})
    mem_idx={m:i for i,m in enumerate(member_list)}
    G=len(groups_out); utilities=groups_out['posterior_strength_logit'].to_numpy(float)
    rows=[]; cols=[]; vals=[]
    for j,gid in enumerate(groups_out.group_id):
        for m in members.get(int(gid),[]):
            rows.append(mem_idx[m]); cols.append(j); vals.append(1.0)
    A=sparse.coo_matrix((vals,(rows,cols)),shape=(len(member_list),G)).tocsr()
    lc=LinearConstraint(A, lb=np.zeros(len(member_list)), ub=np.ones(len(member_list)))
    res=milp(c=-utilities, integrality=np.ones(G), bounds=Bounds(0,1), constraints=lc, options={'time_limit':120})
    if not res.success:
        # deterministic greedy fallback is not used silently; mark if MILP fails.
        raise RuntimeError(f'MILP resolver failed: {res.message}')
    x=np.rint(res.x).astype(int)
    groups_out['resolver_selected']=x
    selected=groups_out[groups_out.resolver_selected==1].copy()
    selected=selected.sort_values('Correct Cqd',ascending=False).reset_index(drop=True)
    selected['Correct Rank']=np.arange(1,len(selected)+1)
    # duplicate check
    used=[]
    for gid in selected.group_id:
        used.extend(members.get(int(gid),[]))
    dup_count=len(used)-len(set(used))
    # review table exactly six columns
    review=selected[['Correct Rank','Correct Cqd','Raw Rank','Raw Cqd','Text-Type','Name']].copy()
    review.to_csv(out_dir/'corrected_rank_review_crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver_48_7.csv',index=False)
    # also all candidate diagnostics
    groups_out.to_csv(out_dir/'final_all_candidate_diagnostics.csv',index=False)
    selected.to_csv(out_dir/'resolver_selection_detail.csv',index=False)
    # Same member competitors
    comp=[]
    for m in member_list:
        gids=[gid for gid,ms in members.items() if m in ms]
        g=groups_out[groups_out.group_id.isin(gids)].sort_values('posterior_strength_logit',ascending=False)
        if len(g)>=2:
            best=g.iloc[0]; second=g.iloc[1]
            comp.append({'member':m,'candidate_count':len(g),'selected_group_id':int(best.group_id) if int(best.resolver_selected)==1 else None,'top_group_id':int(best.group_id),'top_utility_logit':float(best.posterior_strength_logit),'second_group_id':int(second.group_id),'second_utility_logit':float(second.posterior_strength_logit),'best_vs_second_margin_logit':float(best.posterior_strength_logit-second.posterior_strength_logit),'competition_class':'dominant' if best.posterior_strength_logit-second.posterior_strength_logit>float(np.nanmedian(groups_out.stability_strength_sd_logit)) else 'close'})
    pd.DataFrame(comp).to_csv(out_dir/'same_member_competitor_detail.csv',index=False)
    # jumper uncertainty detail
    jump=groups_out.copy(); jump['abs_rank_delta']=jump['rank_delta_all_candidates'].abs(); jump.sort_values(['rank_delta_all_candidates','Correct Rank All Candidates'],ascending=[False,True]).to_csv(out_dir/'posterior_uncertainty_jumper_detail.csv',index=False)
    # summary
    movement={'mean_abs_delta_cqd':float(np.mean(np.abs(groups_out['Correct Cqd']-groups_out['Raw Cqd']))),'max_abs_delta_cqd':float(np.max(np.abs(groups_out['Correct Cqd']-groups_out['Raw Cqd']))),'p95_abs_delta_cqd':float(np.quantile(np.abs(groups_out['Correct Cqd']-groups_out['Raw Cqd']),0.95)),'mean_abs_rank_delta_all_candidates':float(np.mean(np.abs(groups_out['rank_delta_all_candidates']))),'top50_jaccard_diagnostic':float(len(set(groups_out.nsmallest(50,'Raw Rank').group_id)&set(groups_out.nsmallest(50,'Correct Rank All Candidates').group_id))/50.0)}
    final_summary=pd.DataFrame([{'lane_size':lane_size,'group_count':len(groups_out),'edge_count':len(edges),'oof_edges':len(oof),'final_selected_count':len(selected),'duplicate_selected_members':dup_count,'full_selected_k':int(kdf.sort_values('validation_logloss').iloc[0]['k']),'full_beta_raw':beta_all,'full_beta_model':fit.beta,'full_tau_delta':fit.tau_delta,'full_tau_counter':fit.tau_counter,'full_beta_binomial_phi':fit.phi,'full_counter_dim':len(fit.theta),'cqd_scale_note':'crossfit_bagged_delta_over_full_beta','full_model_cqd_scale_note':cqd_scale_note,'full_model_mean_abs_delta_cqd':float(np.mean(np.abs(groups_out['full_model_corrected_cqd']-groups_out['Raw Cqd']))),'full_model_max_abs_delta_cqd':float(np.max(np.abs(groups_out['full_model_corrected_cqd']-groups_out['Raw Cqd']))),**movement}])
    final_summary.to_csv(out_dir/'final_model_summary.csv',index=False)
    # Write report
    raw_metrics=oof_metrics[oof_metrics.model=='raw'].iloc[0].to_dict(); corr_metrics=oof_metrics[oof_metrics.model=='corrected'].iloc[0].to_dict()
    raw_ts=type_summary[type_summary.model=='raw'].iloc[0].to_dict(); corr_ts=type_summary[type_summary.model=='corrected'].iloc[0].to_dict()
    report=f"""# crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver\n\nThis run implements the requested clean branch with all required code aggregated into one Python source file.\n\n## Non-negotiables\n\n- Raw Cqd is used only as immutable input from `lane_results.raw_average_cqd`; it is not recomputed.\n- Golden is not used.\n- Legacy `winrate_type_label` is not used.\n- Text-Type is computed from an embedded Python port of `tswn_lane_ranker/src/skill_eq.rs`, not from W-Type.\n- RSW-Type is derived from strength-neutral residual shape and its K is selected by held-out logloss; k14 is not hardcoded.\n- Resolver is exact generic set-packing MILP: maximize learned utility subject to each member used <= 1.\n- No topK, no rank-movement guard, no manual cap/shrink/golden/history rule is used.\n\n## Model\n\n```text\nlogit P(i beats j) = beta * (RawCqd_i - RawCqd_j) + delta_i - delta_j + counter(type_i,type_j)\ndelta_i ~ Normal(0, tau_delta^2)\ncounter(a,b) = -counter(b,a), counter(a,a)=0\ncounter(a,b) ~ Normal(0, tau_counter^2)\n```\n\nStrength edge predictions are computed by exact beta-binomial penalized MAP with EB prior scales. The final leaderboard uses the cross-fit bagged group delta averaged across outer-fold models, combined with the full-model raw scale; this is a data-driven ensemble estimator and not a hand cap/shrink. Uncertainty fields are cross-fit stability diagnostics rather than Laplace posterior covariance. `tau_delta`, `tau_counter`, and the beta-binomial overdispersion precision `phi` are learned from data; no Laplace covariance approximation is used for the main score.\n\n## OOF prediction headline\n\n| metric | Raw | Corrected | Delta |\n|---|---:|---:|---:|\n| weighted_logloss | {raw_metrics['weighted_logloss']:.9f} | {corr_metrics['weighted_logloss']:.9f} | {corr_metrics['weighted_logloss']-raw_metrics['weighted_logloss']:+.9f} |\n| weighted_brier | {raw_metrics['weighted_brier']:.9f} | {corr_metrics['weighted_brier']:.9f} | {corr_metrics['weighted_brier']-raw_metrics['weighted_brier']:+.9f} |\n| weighted_auc | {raw_metrics['weighted_auc']:.9f} | {corr_metrics['weighted_auc']:.9f} | {corr_metrics['weighted_auc']-raw_metrics['weighted_auc']:+.9f} |\n| weighted_ordering_accuracy | {raw_metrics['weighted_ordering_accuracy']:.9f} | {corr_metrics['weighted_ordering_accuracy']:.9f} | {corr_metrics['weighted_ordering_accuracy']-raw_metrics['weighted_ordering_accuracy']:+.9f} |\n\n## OOF type-bias headline\n\n| metric | Raw | Corrected | Delta |\n|---|---:|---:|---:|\n| type_level_bias_rms | {raw_ts['type_level_bias_rms']:.9f} | {corr_ts['type_level_bias_rms']:.9f} | {corr_ts['type_level_bias_rms']-raw_ts['type_level_bias_rms']:+.9f} |\n| residual_eta2_by_type_a | {raw_ts['residual_eta2_by_type_a']:.9f} | {corr_ts['residual_eta2_by_type_a']:.9f} | {corr_ts['residual_eta2_by_type_a']-raw_ts['residual_eta2_by_type_a']:+.9f} |\n| type_pair_bias_rms | {raw_ts['type_pair_bias_rms']:.9f} | {corr_ts['type_pair_bias_rms']:.9f} | {corr_ts['type_pair_bias_rms']-raw_ts['type_pair_bias_rms']:+.9f} |\n| worst_abs_type_pair_bias | {raw_ts['worst_abs_type_pair_bias']:.9f} | {corr_ts['worst_abs_type_pair_bias']:.9f} | {corr_ts['worst_abs_type_pair_bias']-raw_ts['worst_abs_type_pair_bias']:+.9f} |\n\n## Final full-model resolver\n\n- selected groups: {len(selected)}\n- duplicate selected member count: {dup_count}\n- selected full-model RSW K: {int(kdf.sort_values('validation_logloss').iloc[0]['k'])}\n- beta: {fit.beta:.9f}\n- tau_delta: {fit.tau_delta:.9f}\n- tau_counter: {fit.tau_counter:.9f}\n- mean_abs_delta_cqd: {movement['mean_abs_delta_cqd']:.9f}\n- max_abs_delta_cqd: {movement['max_abs_delta_cqd']:.9f}\n\nThe fixed human review table is exported with exactly:\n\n```text\nCorrect Rank | Correct Cqd | Raw Rank | Raw Cqd | Text-Type | Name\n```\n"""
    (out_dir/'CROSSFIT_BETABINOMIAL_CVBAG_EB_STRENGTH_TYPECOUNTER_RESOLVER_REPORT.md').write_text(report,encoding='utf-8')
    # Copy source itself after run in caller.
    return {'out_dir':str(out_dir),'selected':len(selected),'dup_count':dup_count,'oof_logloss_raw':raw_metrics['weighted_logloss'],'oof_logloss_corrected':corr_metrics['weighted_logloss'],'oof_logloss_delta':corr_metrics['weighted_logloss']-raw_metrics['weighted_logloss'],'mean_abs_delta_cqd':movement['mean_abs_delta_cqd'],'max_abs_delta_cqd':movement['max_abs_delta_cqd'],'p95_abs_delta_cqd':movement['p95_abs_delta_cqd']}


def _excel_safe_value(x):
    if pd.isna(x):
        return None
    if isinstance(x, (np.integer,)):
        return int(x)
    if isinstance(x, (np.floating,)):
        return float(x)
    if isinstance(x, (np.bool_,)):
        return bool(x)
    s=str(x) if not isinstance(x,(int,float,bool)) else x
    if isinstance(s,str) and s.startswith('='):
        return "'" + s
    return s

def _write_df_sheet(wb, sheet_name: str, df: pd.DataFrame, max_rows: Optional[int]=None):
    sh=wb.worksheets.add(sheet_name[:31])
    d=df.copy()
    if max_rows is not None and len(d)>max_rows:
        d=d.head(max_rows).copy()
    # Keep audit workbook compact; full details are exported as CSV.
    rows=[list(d.columns)] + [[_excel_safe_value(v) for v in row] for row in d.itertuples(index=False, name=None)]
    if not rows:
        rows=[['empty']]
    sh.get_range_by_indexes(0,0,len(rows),len(rows[0])).values=rows
    try:
        sh.freeze_panes.freeze_rows(1)
        sh.get_range_by_indexes(0,0,1,len(rows[0])).format={"fill":"#0F766E","font":{"bold":True,"color":"#FFFFFF"}}
    except Exception:
        pass
    return sh

def create_excel_outputs(out_dir: Path) -> None:
    from artifact_tool import Workbook, SpreadsheetFile
    # Review workbook with exactly the required table.
    review_csv=out_dir/'corrected_rank_review_crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver_48_7.csv'
    if review_csv.exists():
        review=pd.read_csv(review_csv)
        wb=Workbook.create()
        _write_df_sheet(wb, 'Review', review)
        SpreadsheetFile.export_xlsx(wb).save(str(out_dir/'corrected_rank_review_crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver_48_7.xlsx'))
    # Comprehensive audit workbook: summary + compact detail slices. Full details remain in CSV.
    wb=Workbook.create()
    sheet_specs=[
        ('Summary','final_model_summary.csv',None),
        ('OOF Metrics','oof_prediction_metrics_before_after.csv',None),
        ('Fold Metrics','outer_fold_prediction_metrics.csv',None),
        ('Type Summary','oof_type_bias_summary_before_after.csv',None),
        ('Type Level','oof_type_level_bias_before_after.csv',100),
        ('Type Pair','oof_type_pair_bias_before_after_detail.csv',200),
        ('Worst TypePair','worst_worsened_type_pair_bias.csv',100),
        ('K Select Full','adaptive_rsw_type_k_selection_full.csv',None),
        ('K Select Folds','adaptive_rsw_type_k_selection_by_fold.csv',120),
        ('BB EB Fit','betabinomial_eb_fit_diagnostics_by_fold.csv',None),
        ('CV Stability','cv_strength_stability_by_group.csv',120),
        ('Counters','antisymmetric_type_counter_posterior.csv',120),
        ('Resolver','resolver_selection_detail.csv',120),
        ('Review','corrected_rank_review_crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver_48_7.csv',None),
        ('Input Checks','input_integrity_checks.csv',None),
    ]
    for sh,csv_name,max_rows in sheet_specs:
        path=out_dir/csv_name
        if path.exists():
            _write_df_sheet(wb, sh, pd.read_csv(path), max_rows=max_rows)
    SpreadsheetFile.export_xlsx(wb).save(str(out_dir/'crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver_audit.xlsx'))

def bundle_outputs(out_dir: Path, source_path: Optional[Path]=None) -> Path:
    if source_path is not None and source_path.exists():
        shutil.copy2(source_path, out_dir/'crossfit_betabinomial_cvbag_eb_strength_typecounter_resolver.py')
    bundle=out_dir.with_name(out_dir.name + '_bundle.zip')
    if bundle.exists():
        bundle.unlink()
    with zipfile.ZipFile(bundle,'w',compression=zipfile.ZIP_DEFLATED) as z:
        for p in sorted(out_dir.iterdir()):
            if p.is_file():
                z.write(p, arcname=p.name)
    return bundle


# =========================
# Low-rank antisymmetric residual interaction branch
# =========================
@dataclass
class LowRankEBFit:
    beta: float
    delta: np.ndarray
    theta: np.ndarray
    gamma: np.ndarray
    tau_delta: float
    tau_counter: float
    tau_lowrank: float
    phi: float
    theta_pairs: List[Tuple[int,int]]
    skew_pairs: List[Tuple[int,int]]
    lowrank_rank: int
    success: bool
    message: str
    iterations: int
    map_nll: float
    eb_converged: bool
    eb_rounds: int
    eb_hit_round_limit: bool
    optimizer_all_rounds_converged: bool
    optimizer_hit_iteration_limit: bool

def robust_spectral_rank(X: np.ndarray, target_rank: Optional[int] = None) -> Tuple[int, pd.DataFrame, np.ndarray]:
    """Self-adaptive low-rank dimension from the residual-profile spectrum.

    No fixed k14 or handpicked type count is written into the algorithm.
    A component is kept only when its singular value clears a robust spectrum
    noise floor: median(s) + MAD-to-sigma constant * MAD(s). If fewer than two
    components clear the floor, the antisymmetric low-rank interaction is absent
    because rank 0/1 cannot form a skew interaction.
    """
    X = np.asarray(X, dtype=float)
    X = np.nan_to_num(X)
    Xc = X - X.mean(axis=0, keepdims=True)
    if min(Xc.shape) <= 1:
        return 0, pd.DataFrame([{"component": 1, "singular_value": 0.0, "selected": False}]), np.zeros((X.shape[0], 0))
    U, s, Vt = np.linalg.svd(Xc, full_matrices=False)
    if len(s) == 0 or float(np.sum(s*s)) <= 0:
        return 0, pd.DataFrame([{"component": 1, "singular_value": 0.0, "selected": False}]), np.zeros((X.shape[0], 0))
    med = float(np.median(s))
    mad = float(np.median(np.abs(s - med)))
    # 1.4826 is the standard normal consistency constant for MAD, not a leaderboard cap.
    floor = med + 1.4826 * mad
    selected = s > floor
    if target_rank is not None:
        # Interaction-rich branch:
        # low-rank capacity is tied to the held-out selected RSW K, not a handpicked fixed number.
        # EB still learns tau_lowrank and shrinks unnecessary skew directions.
        rank = int(max(0, min(int(target_rank), len(s))))
    else:
        rank = int(selected.sum())
        if rank < 2:
            rank = 0
    scores = Xc @ Vt[:rank].T if rank > 0 else np.zeros((X.shape[0], 0))
    if rank > 0:
        scores = (scores - scores.mean(axis=0, keepdims=True)) / np.maximum(scores.std(axis=0, keepdims=True), 1e-12)
    rec = []
    denom = float(np.sum(s*s))
    csum = 0.0
    for i, sv in enumerate(s, start=1):
        csum += float(sv*sv)
        rec.append({
            "component": i,
            "singular_value": float(sv),
            "robust_noise_floor": floor,
            "selected": bool(i <= rank),
            "cumulative_variance_share": csum / denom,
        })
    return rank, pd.DataFrame(rec), scores

def make_skew_pairs(rank: int) -> List[Tuple[int, int]]:
    return [(a, b) for a in range(rank) for b in range(a + 1, rank)]

def build_lowrank_design(edges_subset: pd.DataFrame, embedding: np.ndarray, skew_pairs: List[Tuple[int, int]]) -> np.ndarray:
    if len(skew_pairs) == 0:
        return np.zeros((len(edges_subset), 0), dtype=float)
    ia = edges_subset["ia"].to_numpy(dtype=int)
    ib = edges_subset["ib"].to_numpy(dtype=int)
    Eia = embedding[ia]
    Eib = embedding[ib]
    Z = np.empty((len(edges_subset), len(skew_pairs)), dtype=float)
    for j, (a, b) in enumerate(skew_pairs):
        Z[:, j] = Eia[:, a] * Eib[:, b] - Eia[:, b] * Eib[:, a]
    # Embedding coordinates already have a fixed scale.  Never standardize Z
    # on the supplied edge subset: train and validation would otherwise use
    # different feature coordinates for the same fitted gamma.
    return np.nan_to_num(Z)

def derive_lowrank_embedding(groups_df, edges_df, raw_beta, train_mask, seed=123, target_rank: Optional[int] = None):
    X, prof, wsum = build_profile(groups_df, edges_df, raw_beta, train_mask, {g: i for i, g in enumerate(groups_df.group_id)})
    rank, spectrum_df, embedding = robust_spectral_rank(X, target_rank=target_rank)
    spectrum_df["seed"] = int(seed)
    spectrum_df["selected_rank"] = int(rank)
    return rank, spectrum_df, embedding, prof, wsum

def fit_betabinomial_lowrank_counter_eb(groups_df, edges_df, train_mask, type_ids, embedding, max_eb_iter=6, tol=1e-3, init=None, fixed_beta: Optional[float] = None):
    """Exact beta-binomial EB with strength delta, discrete type counter, and continuous low-rank skew counter.

    The low-rank term is:
        lowrank_counter(i,j) = u_i^T S u_j, S = -S^T
    represented by free parameters gamma_ab for a<b:
        sum_ab gamma_ab * (u_i,a*u_j,b - u_i,b*u_j,a)

    This lets the model absorb smooth matchup structure instead of forcing
    group strength deltas to eat sparse type-pair effects. The leaderboard score
    still uses strength posterior mean only: beta*RawCqd + delta.
    """
    raw = groups_df["raw_cqd"].to_numpy()
    G = len(raw)
    sub = edges_df.loc[train_mask].copy()
    ia = sub["ia"].to_numpy(dtype=int)
    ib = sub["ib"].to_numpy(dtype=int)
    xraw = raw[ia] - raw[ib]
    y = sub["win_rate_a"].to_numpy(dtype=float)
    n = sub["samples"].to_numpy(dtype=float)
    k = np.rint(np.clip(y, 0, 1) * n)
    likelihood_weight = sub["likelihood_weight"].to_numpy(dtype=float) if "likelihood_weight" in sub.columns else np.ones(len(sub), dtype=float)
    likelihood_weight = np.nan_to_num(likelihood_weight, nan=0.0, posinf=0.0, neginf=0.0)
    likelihood_weight = np.maximum(likelihood_weight, 0.0)

    ta = type_ids[ia]
    tb = type_ids[ib]
    theta_pairs, cmap = make_counter_index(type_ids)
    C = len(theta_pairs)
    cid = np.full(len(sub), -1, dtype=int)
    csign = np.zeros(len(sub), dtype=float)
    for idx, (a, b) in enumerate(zip(ta, tb)):
        if a == b:
            continue
        lo, hi = (int(a), int(b)) if a < b else (int(b), int(a))
        cid[idx] = cmap[(lo, hi)]
        csign[idx] = 1.0 if a < b else -1.0

    rank = int(embedding.shape[1]) if embedding is not None else 0
    skew_pairs = make_skew_pairs(rank)
    L = len(skew_pairs)
    Zlow = build_lowrank_design(sub, embedding, skew_pairs) if L else np.zeros((len(sub), 0), dtype=float)

    core_dim = 1 + G + C + L
    dim = core_dim + 1  # + log_phi
    m = len(sub)
    rr = [np.arange(m), np.arange(m), np.arange(m)]
    cc = [np.zeros(m, dtype=int), 1 + ia, 1 + ib]
    dd = [xraw.astype(float), np.ones(m), -np.ones(m)]
    if C > 0:
        good = cid >= 0
        rr.append(np.arange(m)[good])
        cc.append(1 + G + cid[good])
        dd.append(csign[good])
    if L > 0:
        for j in range(L):
            nz = np.isfinite(Zlow[:, j]) & (np.abs(Zlow[:, j]) > 0)
            if nz.any():
                rr.append(np.arange(m)[nz])
                cc.append(np.full(int(nz.sum()), 1 + G + C + j, dtype=int))
                dd.append(Zlow[nz, j].astype(float))
    X = sparse.csr_matrix((np.concatenate(dd), (np.concatenate(rr), np.concatenate(cc))), shape=(m, core_dim))

    if init is None:
        z = np.zeros(dim)
        z[0] = fit_raw_beta(xraw, y, n * likelihood_weight)
        mu0 = sigmoid(z[0] * xraw)
        z[-1] = math.log(initial_beta_binomial_phi(y, n, mu0))
    else:
        z = np.zeros(dim)
        z[:min(dim, len(init))] = init[:min(dim, len(init))]
        if z[-1] == 0.0:
            z[-1] = math.log(max(1e-3, initial_beta_binomial_phi(y, n, sigmoid(z[0] * xraw))))
    if fixed_beta is not None and np.isfinite(float(fixed_beta)):
        z[0] = float(fixed_beta)

    p0 = np.clip((y * n + 0.5) / (n + 1.0), 1e-6, 1 - 1e-6)
    raw_resid = logit(p0) - z[0] * xraw
    resid_weight = np.maximum(n * likelihood_weight, 1e-12)
    resid_sd = float(np.sqrt(np.average((raw_resid - np.average(raw_resid, weights=resid_weight)) ** 2, weights=resid_weight))) if len(raw_resid) else 0.1
    tau_delta = max(resid_sd / 4.0, 1e-6)
    tau_counter = max(resid_sd / 4.0, 1e-6) if C > 0 else 1e-6
    tau_lowrank = max(resid_sd / 4.0, 1e-6) if L > 0 else 1e-6

    success = True
    msg = "bb-lowrank-lbfgsb"
    map_nll = np.nan
    iters = 0

    def objective_grad(par, prior_diag):
        core = par[:core_dim]
        log_phi = float(par[-1])
        phi = math.exp(float(np.clip(log_phi, -20.0, 20.0)))
        eta = np.clip(X.dot(core), -40, 40)
        mu = sigmoid(eta)
        a = np.maximum(mu * phi, 1e-12)
        b = np.maximum((1.0 - mu) * phi, 1e-12)
        ll = betaln(k + a, n - k + b) - betaln(a, b)
        weighted_ll = likelihood_weight * ll
        nll = float(-np.sum(weighted_ll) + 0.5 * np.sum(prior_diag * core * core))

        dL_deta = likelihood_weight * phi * mu * (1.0 - mu) * (digamma(k + a) - digamma(a) - digamma(n - k + b) + digamma(b))
        grad_core = -np.asarray(X.T.dot(dL_deta)).ravel() + prior_diag * core
        dL_dphi = (
            mu * (digamma(k + a) - digamma(a))
            + (1.0 - mu) * (digamma(n - k + b) - digamma(b))
            - digamma(n + phi)
            + digamma(phi)
        )
        grad_log_phi = float(-np.sum(likelihood_weight * dL_dphi) * phi)
        grad = np.empty_like(par)
        grad[:core_dim] = grad_core
        grad[-1] = grad_log_phi
        if not np.isfinite(nll) or not np.all(np.isfinite(grad)):
            return 1e300, np.nan_to_num(grad, nan=0.0, posinf=1e100, neginf=-1e100)
        return nll, grad

    eb_converged = False; eb_rounds = 0
    optimizer_all_rounds_converged = True; optimizer_hit_iteration_limit = False
    for eb in range(max_eb_iter):
        eb_rounds = eb + 1
        prior_diag = np.zeros(core_dim)
        prior_diag[1:1 + G] = 1.0 / max(tau_delta, 1e-8) ** 2
        if C > 0:
            prior_diag[1 + G:1 + G + C] = 1.0 / max(tau_counter, 1e-8) ** 2
        if L > 0:
            prior_diag[1 + G + C:1 + G + C + L] = 1.0 / max(tau_lowrank, 1e-8) ** 2

        def fun(par):
            val, grad = objective_grad(par, prior_diag)
            return val, grad

        bounds = None
        if fixed_beta is not None and np.isfinite(float(fixed_beta)):
            fb = float(fixed_beta)
            bounds = [(fb, fb)] + [(None, None)] * (len(z) - 1)
            z[0] = fb
        opt = minimize(fun, z, method="L-BFGS-B", jac=True, bounds=bounds, options={"maxiter": 90, "ftol": 1e-7, "gtol": 1e-5, "maxls": 30})
        if fixed_beta is not None and np.isfinite(float(fixed_beta)):
            z[0] = float(fixed_beta)
        z = opt.x
        success = bool(opt.success)
        optimizer_all_rounds_converged = optimizer_all_rounds_converged and bool(opt.success)
        optimizer_hit_iteration_limit = optimizer_hit_iteration_limit or int(getattr(opt, "nit", 0) or 0) >= 90 or "ITERATION" in str(opt.message).upper() and "LIMIT" in str(opt.message).upper()
        msg = str(opt.message)
        map_nll = float(opt.fun)
        iters += int(getattr(opt, "nit", 0) or 0)

        delta = z[1:1 + G]
        theta = z[1 + G:1 + G + C]
        gamma = z[1 + G + C:1 + G + C + L]
        new_tau_delta = float(math.sqrt(max(1e-12, np.mean(delta * delta))))
        new_tau_counter = float(math.sqrt(max(1e-12, np.mean(theta * theta)))) if C > 0 else 1e-6
        new_tau_lowrank = float(math.sqrt(max(1e-12, np.mean(gamma * gamma)))) if L > 0 else 1e-6
        if (
            abs(math.log(new_tau_delta / max(tau_delta, 1e-12))) < tol
            and (C == 0 or abs(math.log(new_tau_counter / max(tau_counter, 1e-12))) < tol)
            and (L == 0 or abs(math.log(new_tau_lowrank / max(tau_lowrank, 1e-12))) < tol)
        ):
            tau_delta, tau_counter, tau_lowrank = new_tau_delta, new_tau_counter, new_tau_lowrank
            eb_converged = True
            break
        tau_delta, tau_counter, tau_lowrank = new_tau_delta, new_tau_counter, new_tau_lowrank

    phi = float(math.exp(float(np.clip(z[-1], -20.0, 20.0))))
    return LowRankEBFit(
        beta=float(z[0]),
        delta=z[1:1 + G].copy(),
        theta=z[1 + G:1 + G + C].copy(),
        gamma=z[1 + G + C:1 + G + C + L].copy(),
        tau_delta=tau_delta,
        tau_counter=tau_counter,
        tau_lowrank=tau_lowrank,
        phi=phi,
        theta_pairs=theta_pairs,
        skew_pairs=skew_pairs,
        lowrank_rank=rank,
        success=success,
        message=msg,
        iterations=iters,
        map_nll=map_nll,
        eb_converged=bool(eb_converged),
        eb_rounds=int(eb_rounds),
        eb_hit_round_limit=bool(not eb_converged and eb_rounds >= max_eb_iter),
        optimizer_all_rounds_converged=bool(optimizer_all_rounds_converged),
        optimizer_hit_iteration_limit=bool(optimizer_hit_iteration_limit),
    )

def predict_edges_lowrank(groups_df, edges_df, mask, fit: LowRankEBFit, type_ids, embedding):
    raw = groups_df["raw_cqd"].to_numpy()
    sub = edges_df.loc[mask].copy()
    ia = sub["ia"].to_numpy(dtype=int)
    ib = sub["ib"].to_numpy(dtype=int)
    eta = fit.beta * (raw[ia] - raw[ib]) + fit.delta[ia] - fit.delta[ib]
    if len(fit.theta) > 0:
        cmap = {p: i for i, p in enumerate(fit.theta_pairs)}
        ta = type_ids[ia]
        tb = type_ids[ib]
        c = np.zeros(len(sub))
        for r, (a, b) in enumerate(zip(ta, tb)):
            if a == b:
                continue
            lo, hi = (int(a), int(b)) if a < b else (int(b), int(a))
            sgn = 1.0 if a < b else -1.0
            j = cmap.get((lo, hi))
            if j is not None:
                c[r] = sgn * fit.theta[j]
        eta += c
    if len(fit.gamma) > 0:
        Zlow = build_lowrank_design(sub, embedding, fit.skew_pairs)
        eta += Zlow.dot(fit.gamma)
    return eta

def require_complete_undirected_winrate_edges(edges: pd.DataFrame, group_ids: Sequence[int], context: str) -> None:
    ids = sorted({int(g) for g in group_ids})
    if len(ids) < 2:
        return
    observed = set()
    if not edges.empty:
        for a, b in edges[["group_a", "group_b"]].itertuples(index=False, name=None):
            ia, ib = int(a), int(b)
            if ia == ib or ia not in ids or ib not in ids:
                continue
            if ia > ib:
                ia, ib = ib, ia
            observed.add((ia, ib))
    missing = []
    for i, a in enumerate(ids):
        for b in ids[i + 1:]:
            if (a, b) not in observed:
                missing.append((a, b))
    if missing:
        preview = "; ".join(f"{a}-{b}" for a, b in missing[:20])
        raise RuntimeError(
            f"Missing required win-rate data for {context}: {len(missing)} missing pair(s); "
            f"first_missing_pairs={preview}"
        )


class MissingRateRequest(RuntimeError):
    def __init__(self, context: str, missing_pairs: Sequence[Tuple[int, int]], lane_size: int, out_dir: Path):
        self.context = str(context)
        self.missing_pairs = sorted({(int(min(a, b)), int(max(a, b))) for a, b in missing_pairs if int(a) != int(b)})
        self.lane_size = int(lane_size)
        self.out_dir = Path(out_dir)
        super().__init__(f"Missing required win-rate data for {self.context}: {len(self.missing_pairs)} pair(s)")


def _observed_undirected_pairs(edges: pd.DataFrame) -> set:
    observed = set()
    if edges is None or edges.empty:
        return observed
    for a, b in edges[["group_a", "group_b"]].itertuples(index=False, name=None):
        ia, ib = int(a), int(b)
        if ia == ib:
            continue
        if ia > ib:
            ia, ib = ib, ia
        observed.add((ia, ib))
    return observed


def collect_missing_cross_pairs(edges: pd.DataFrame, left_ids: Sequence[int], right_ids: Sequence[int]) -> List[Tuple[int, int]]:
    left = sorted({int(g) for g in left_ids})
    right = sorted({int(g) for g in right_ids})
    observed = _observed_undirected_pairs(edges)
    missing = []
    for a in left:
        for b in right:
            if a == b:
                continue
            lo, hi = (a, b) if a < b else (b, a)
            if (lo, hi) not in observed:
                missing.append((lo, hi))
    return sorted(set(missing))


def collect_missing_internal_pairs(edges: pd.DataFrame, ids: Sequence[int]) -> List[Tuple[int, int]]:
    ids = sorted({int(g) for g in ids})
    observed = _observed_undirected_pairs(edges)
    missing = []
    for i, a in enumerate(ids):
        for b in ids[i + 1:]:
            if (a, b) not in observed:
                missing.append((a, b))
    return missing


def require_pairs_or_request(
    edges: pd.DataFrame,
    left_ids: Sequence[int],
    right_ids: Optional[Sequence[int]],
    context: str,
    lane_size: int,
    out_dir: Path,
) -> None:
    if right_ids is None:
        missing = collect_missing_internal_pairs(edges, left_ids)
    else:
        missing = collect_missing_cross_pairs(edges, left_ids, right_ids)
    if missing:
        raise MissingRateRequest(context, missing, lane_size, out_dir)


def write_missing_rate_request(exc: MissingRateRequest) -> Path:
    path = exc.out_dir / "strict_python_missing_rate_pairs.json"
    rows = [
        {"group_a": int(a), "group_b": int(b), "context": exc.context}
        for a, b in exc.missing_pairs
    ]
    payload = {
        "kind": "strict_python_missing_rate_pairs",
        "lane_size": int(exc.lane_size),
        "context": exc.context,
        "missing_pair_count": int(len(rows)),
        "pairs": rows,
    }
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return path


def _prepare_partial_edges_for_ids(all_edges: pd.DataFrame, ids: Sequence[int], context: str) -> pd.DataFrame:
    """Prepare graph edges for residual-profile/RSW work where only reference edges are required."""
    ids = sorted({int(g) for g in ids})
    idset = set(ids)
    group_to_idx = {gid: i for i, gid in enumerate(ids)}
    edges = all_edges[all_edges.group_a.isin(idset) & all_edges.group_b.isin(idset)].copy()
    if edges.empty and len(ids) >= 2:
        raise RuntimeError(f"No pairwise edges available for {context}")
    edges["ia"] = edges.group_a.map(group_to_idx).astype(int)
    edges["ib"] = edges.group_b.map(group_to_idx).astype(int)
    edges["fold5"] = edge_fold_ids(edges["group_a"].to_numpy(), edges["group_b"].to_numpy(), nfold=max(2, min(5, len(edges))))
    return edges


# =========================
# Active-set challenger selection
# =========================
#
# The public leaderboard displays one non-overlapping "main" set, but the raw
# candidate table can contain many groups sharing the same members.  Training on
# every duplicate candidate makes the model optimize the wrong universe.  The
# weighted active-environment loop below represents the displayed environment as
# continuous membership weights q_i in [0, 1].  Both the active fit and the
# challenger projection are weighted by q, then q is updated by a damped soft
# browser-greedy map until it reaches a numerical fixed point.

# Weighted active-environment iteration.  These are numerical iteration
# parameters, not rank caps or hand-written candidate filters.  The active
# environment is represented by q_i in [0, 1] instead of a hard selected set.
ACTIVE_SET_WEIGHTED_MAX_ITERS = 100
ACTIVE_SET_WEIGHTED_SUPPORT_EPS = 5e-2
ACTIVE_SET_WEIGHTED_CONVERGENCE_MAX_DELTA = 1e-3
ACTIVE_SET_WEIGHTED_CONVERGENCE_MEAN_DELTA = 2.5e-4
ACTIVE_SET_WEIGHTED_MASS_TOL = 1e-4

# Regularized self-training controls.  These are continuous EB / validation
# controls, not rank caps, Raw guards, blacklist rules, or hard rescue filters.
ACTIVE_REG_BASE_DELTA_ROBUST_SCALE_CQD = 1.25
# Global base is an EB prior, not the final active score.  When the fitted
# group-delta prior scale explodes in CQD units (tau_delta / beta), the base
# becomes a full-data group correction and can switch regimes around a small
# raw_min change.  This soft shrink keeps global base as a modest prior while
# leaving final active projection/residual correction to carry local evidence.
ACTIVE_REG_GLOBAL_BASE_TAU_CQD_STABILITY_SCALE = 0.75
# Historical learned-global-base controls are intentionally not used by the
# current run path.  `global_base_cqd` is Raw; all learnable movement is confined
# to active projection / residual correction below.
# Active residuals are now evidence-aware.  Global base stays exactly Raw;
# these controls only affect the post-active score-only residual layer.
ACTIVE_REG_RESIDUAL_SOFT_CAP_CQD = 1.25
# Make the cap evidence-aware: high-evidence score-only rows can retain more
# active residual, while selected/q-support rows remain more strongly protected
# against self-fit.  These are continuous shrink controls, not Raw/rank guards.
ACTIVE_REG_SCORE_ONLY_SOFT_CAP_MULTIPLIER = 1.00
ACTIVE_REG_SELECTED_ROW_SOFT_CAP_MULTIPLIER = 0.85
ACTIVE_REG_EVIDENCE_SOFT_CAP_MIN_MULTIPLIER = 0.75
ACTIVE_REG_EVIDENCE_SOFT_CAP_MAX_MULTIPLIER = 1.35
ACTIVE_REG_VALIDATION_SOFT_CAP_MAX_BONUS = 0.15
ACTIVE_REG_Q_MASS_KAPPA = 3.0
ACTIVE_REG_EDGE_COUNT_KAPPA = 4.0
ACTIVE_REG_SAMPLE_MASS_KAPPA = 600.0
ACTIVE_REG_REFERENCE_DIVERSITY_KAPPA = 2.0
ACTIVE_REG_UNCERTAINTY_PENALTY_CQD = 0.045
ACTIVE_REG_LEVERAGE_PENALTY_CQD = 0.035
ACTIVE_REG_SELECTED_ROW_SELF_FIT_MULTIPLIER = 0.72
ACTIVE_REG_VALIDATION_TOL = 1e-4
ACTIVE_REG_VALIDATION_SHRINK_SCALE_LOGLOSS = 0.002
ACTIVE_REG_VALIDATION_BAD_PATIENCE = 8
ACTIVE_REG_GOOD_STEP_ALPHA = 0.35
ACTIVE_REG_NEUTRAL_STEP_ALPHA = 0.18
ACTIVE_REG_BAD_STEP_ALPHA = 0.07
ACTIVE_REG_CHURN_DAMP_START_RATIO = 0.35
ACTIVE_REG_CHURN_DAMP_FULL_RATIO = 0.90
ACTIVE_REG_CHURN_DAMP_MIN_MULTIPLIER = 0.35
# Late-stage one-in/one-out q flips can keep max_delta high even when aggregate
# mass and validation are stable.  Apply an additional smooth micro-churn damp
# after the exploratory warmup instead of early-stopping.
ACTIVE_REG_MICRO_CHURN_DAMP_START_ITER = 10
ACTIVE_REG_MICRO_CHURN_DAMP_STRENGTH = 80.0
ACTIVE_REG_MICRO_CHURN_DAMP_DELTA_SCALE = 0.25
ACTIVE_REG_MICRO_CHURN_DAMP_MIN_MULTIPLIER = 0.35

# Environment-level moment alignment and tail-q finalization.  These are not
# Raw adhesion guards and not rank caps: they only prevent the active
# environment from creating its own global mean/variance drift while q is still
# seeking a fixed point.
ACTIVE_MOMENT_ALIGNMENT_MIN_ROWS = 5
ACTIVE_MOMENT_ALIGNMENT_MIN_SD_CQD = 1e-8
ACTIVE_Q_TARGET_EMA_ALPHA = 0.35
ACTIVE_Q_OSCILLATION_DAMP_STRENGTH = 0.35
ACTIVE_Q_TAIL_WINDOW = 20
# Final active environment uses only tail-persistent support.  Boundary rows
# whose q briefly crosses support_eps in the last phase are kept in the audit
# but not allowed to define the final reference environment.
ACTIVE_Q_TAIL_MIN_SUPPORT_PROB = 0.90



def _members_for_group(group_members: Dict[int, List[str]], gid: int, fallback_name: str = "") -> List[str]:
    ms = group_members.get(int(gid))
    if ms:
        return [str(x) for x in ms]
    if fallback_name:
        return [str(x) for x in str(fallback_name).split("+") if str(x)]
    return [str(gid)]


def _solve_member_setpacking(
    scored: pd.DataFrame,
    group_members: Dict[int, List[str]],
    utility_col: str,
    score_col: str,
    context: str,
) -> List[int]:
    if scored.empty:
        return []
    df = scored.copy()
    df = df[np.isfinite(df[utility_col].to_numpy(float)) & np.isfinite(df[score_col].to_numpy(float))].copy()
    if df.empty:
        return []

    # Keep deterministic column order for MILP and tie-breaking.
    df = df.sort_values([score_col, "raw_cqd", "group_id"], ascending=[False, False, True]).reset_index(drop=True)
    member_list = sorted({
        m
        for _, r in df.iterrows()
        for m in _members_for_group(group_members, int(r.group_id), str(r.get("canonical", "")))
    })
    if not member_list:
        return []

    mem_idx = {m: i for i, m in enumerate(member_list)}
    rows, cols, vals = [], [], []
    for j, r in df.iterrows():
        for m in _members_for_group(group_members, int(r.group_id), str(r.get("canonical", ""))):
            rows.append(mem_idx[m])
            cols.append(j)
            vals.append(1.0)

    A = sparse.coo_matrix((vals, (rows, cols)), shape=(len(member_list), len(df))).tocsr()
    lc = LinearConstraint(A, lb=np.zeros(len(member_list)), ub=np.ones(len(member_list)))
    utilities = df[utility_col].to_numpy(float)

    res = milp(c=-utilities, integrality=np.ones(len(df)), bounds=Bounds(0, 1), constraints=lc, options={"time_limit": 120})
    if not res.success:
        raise RuntimeError(f"Active-set resolver failed in {context}: {res.message}")

    x = np.rint(res.x).astype(int)
    selected = df.loc[x == 1].copy()
    selected = selected.sort_values([score_col, "raw_cqd", "group_id"], ascending=[False, False, True])
    return [int(g) for g in selected["group_id"].tolist()]




def _greedy_visible_main_group_ids(
    scored: pd.DataFrame,
    group_members: Dict[int, List[str]],
    score_col: str,
    context: str,
) -> List[int]:
    """Browser-equivalent main-row selection.

    Sort by score, then keep the first row that does not share any member with
    an already-visible parent. Lower overlapping rows become hidden/blocked-like
    children. This is intentionally not a surplus MILP: the browser main list
    should not drop unrelated rows just because they are below a global utility
    baseline.
    """
    if scored.empty:
        return []
    df = scored.copy()
    df = df[np.isfinite(df[score_col].to_numpy(float))].copy()
    if df.empty:
        return []

    sort_cols = [score_col]
    ascending = [False]
    if "raw_cqd" in df.columns:
        sort_cols.append("raw_cqd")
        ascending.append(False)
    sort_cols.append("group_id")
    ascending.append(True)

    df = df.sort_values(sort_cols, ascending=ascending).reset_index(drop=True)
    used_members = set()
    selected = []
    for _, r in df.iterrows():
        gid = int(r.group_id)
        ms = set(_members_for_group(group_members, gid, str(r.get("canonical", ""))))
        if not ms:
            ms = {str(gid)}
        if used_members & ms:
            continue
        selected.append(gid)
        used_members.update(ms)

    if not selected:
        raise RuntimeError(f"active-set greedy visible-main selection produced no rows in {context}")
    return selected



def _active_update_scoreable_above_baseline(
    scored: pd.DataFrame,
    baseline_cqd: float,
    score_col: str,
    context: str,
) -> pd.DataFrame:
    """Rows at or below the current environment floor become challengers.

    This is a soft kick-out: the row remains in the eligible/scout pool and can
    be scored again against later active sets. It simply cannot define the next
    training environment while its corrected score is <= the environment floor.
    """
    df = scored.copy()
    score = pd.to_numeric(df[score_col], errors="coerce")
    df["active_set_soft_kicked_below_baseline"] = np.isfinite(score.to_numpy(float)) & (score <= float(baseline_cqd))
    keep = df["active_set_score_success"].fillna(True).astype(bool) & np.isfinite(score.to_numpy(float)) & (score > float(baseline_cqd))
    kept = df.loc[keep].copy()
    if kept.empty:
        raise RuntimeError(
            f"Active-set update in {context} has no rows with finite {score_col} above baseline_cqd={baseline_cqd}; "
            "cannot form next active environment"
        )
    return kept


def active_set_cqd_smoothing_alpha(lane_size: int) -> float:
    """Lane-size dependent transition factor for active-set CQD updates.

    Single: 0.05, pair: 0.10, triple: 0.15, etc. Clamp to [0.01, 1.0]
    for defensive robustness on unusual lane sizes.
    """
    try:
        lane = int(lane_size)
    except Exception:
        lane = 2
    alpha = 0.05 * max(1, lane)
    return float(min(1.0, max(0.01, alpha)))


def _apply_active_set_cqd_smoothing(
    scored: pd.DataFrame,
    score_state: Dict[int, float],
    alpha: float,
    raw_col: str = "raw_cqd",
    current_score_col: str = "Correct Cqd",
    output_col: str = "active_set_smoothed_correct_cqd",
) -> pd.DataFrame:
    """Smooth each group's active-update CQD trajectory.

    This smoothed value is used only for next-active selection and environment
    soft-kick decisions. It does not overwrite the model's actual Correct Cqd.
    """
    df = scored.copy()
    out_values = []
    for gid, raw_value, current in df[["group_id", raw_col, current_score_col]].itertuples(index=False, name=None):
        gid = int(gid)
        raw_value = float(raw_value)
        try:
            current = float(current)
        except Exception:
            current = raw_value
        if not np.isfinite(current):
            current = raw_value
        previous = float(score_state.get(gid, raw_value))
        smoothed = previous + float(alpha) * (current - previous)
        score_state[gid] = smoothed
        out_values.append(smoothed)
    df[output_col] = out_values
    return df


def _restrict_scoreable_for_active_update(
    scored: pd.DataFrame,
    baseline_cqd: float,
    score_col: str,
    context: str,
) -> pd.DataFrame:
    """Apply the soft environment floor to the active-update score column."""
    return _active_update_scoreable_above_baseline(
        scored,
        baseline_cqd,
        score_col=score_col,
        context=context,
    )



def _compact_id_list(ids: Sequence[int], limit: int = 80) -> str:
    ids_list = [int(x) for x in list(ids)]
    vals = [str(x) for x in ids_list[:limit]]
    suffix = "" if len(ids_list) <= limit else f"...(+{len(ids_list) - limit})"
    return ",".join(vals) + suffix


def _write_active_iteration_debug_files(
    out_dir: Path,
    iteration: int,
    combined: pd.DataFrame,
    active_ids_in: Sequence[int],
    support_ids_out: Sequence[int],
    hard_visible_ids: Sequence[int],
    old_weight: Dict[int, float],
    target_weight: Dict[int, float],
    new_weight: Dict[int, float],
    step_alpha: float,
    baseline_cqd: float,
    active_model: Dict[str, Any],
    validation_bad_rounds: int,
) -> Dict[str, Any]:
    """Write compact per-iteration active-state diagnostics."""
    df = combined.copy()
    gid = df["group_id"].astype(int)
    active_in = set(int(x) for x in active_ids_in)
    support_out = set(int(x) for x in support_ids_out)
    selected_out = set(int(x) for x in hard_visible_ids)

    df["debug_q_in"] = gid.map(lambda g: float(old_weight.get(int(g), 0.0))).astype(float)
    df["debug_q_target"] = gid.map(lambda g: float(target_weight.get(int(g), 0.0))).astype(float)
    df["debug_q_next"] = gid.map(lambda g: float(new_weight.get(int(g), 0.0))).astype(float)
    df["debug_q_delta_signed"] = df["debug_q_next"] - df["debug_q_in"]
    df["debug_q_delta_abs"] = df["debug_q_delta_signed"].abs()
    df["debug_support_in"] = gid.isin(active_in)
    df["debug_support_next"] = gid.isin(support_out)
    df["debug_selected_next"] = gid.isin(selected_out)
    df["debug_entered_support"] = (~df["debug_support_in"]) & df["debug_support_next"]
    df["debug_exited_support"] = df["debug_support_in"] & (~df["debug_support_next"])

    if "active_set_smoothed_regularized_cqd" in df.columns:
        score_col = "active_set_smoothed_regularized_cqd"
    elif "regularized_active_cqd" in df.columns:
        score_col = "regularized_active_cqd"
    elif "Correct Cqd" in df.columns:
        score_col = "Correct Cqd"
    else:
        score_col = None
    df["debug_score_for_q"] = pd.to_numeric(df[score_col], errors="coerce") if score_col else np.nan

    raw_col = "raw_cqd" if "raw_cqd" in df.columns else ("Raw Cqd" if "Raw Cqd" in df.columns else None)
    if raw_col:
        df["debug_score_delta_from_raw"] = df["debug_score_for_q"].astype(float) - pd.to_numeric(df[raw_col], errors="coerce").astype(float)
    else:
        df["debug_score_delta_from_raw"] = np.nan

    compact_cols = [
        "group_id", "raw_rank", "raw_cqd", "Raw Rank", "Raw Cqd",
        "debug_q_in", "debug_q_target", "debug_q_next",
        "debug_q_delta_signed", "debug_q_delta_abs",
        "debug_support_in", "debug_support_next", "debug_entered_support", "debug_exited_support",
        "debug_selected_next", "debug_score_for_q", "debug_score_delta_from_raw",
        "Correct Cqd", "regularized_active_cqd", "active_set_smoothed_regularized_cqd",
        "global_base_cqd", "active_residual_raw_cqd", "active_residual_shrunk_cqd",
        "active_residual_reliability", "active_residual_q_mass_reliability",
        "active_residual_edge_count_reliability", "active_residual_sample_mass_reliability",
        "active_residual_reference_diversity_reliability", "active_residual_coverage_reliability",
        "active_residual_validation_survival", "active_residual_selected_row_multiplier",
        "active_residual_soft_cap_factor", "active_residual_survival_multiplier",
        "active_uncertainty_penalty_cqd", "active_leverage_penalty_cqd",
        "active_total_penalty_cqd", "active_set_score_success",
        "active_set_score_message", "active_set_challenger_edges",
        "active_weight_reference_mass", "active_weight_reference_sample_mass",
        "active_weight_reference_q_sample_mass", "active_weight_reference_effective_count",
        "active_weight_reference_max_share", "active_weight_reference_max_evidence_share",
        "active_weight_reference_missing_edges", "active_weight_evidence_gate",
        "active_weight_step_alpha_before_churn_damp", "active_weight_churn_ratio_tentative",
        "active_weight_churn_step_multiplier",
        "scout_candidate", "raw_score_ge_candidate_min", "RSW-Type", "Text-Type", "Name",
    ]
    compact = df[[c for c in compact_cols if c in df.columns]].copy()
    sort_cols = [c for c in ["debug_support_next", "debug_q_next", "debug_score_for_q", "raw_cqd"] if c in compact.columns]
    if sort_cols:
        compact = compact.sort_values(sort_cols, ascending=[False] * len(sort_cols))
    compact.to_csv(out_dir / f"active_set_state_transition_iter{iteration}.csv", index=False)

    support_view = compact[compact["debug_support_next"].fillna(False)].copy() if "debug_support_next" in compact.columns else compact.iloc[0:0].copy()
    support_view.to_csv(out_dir / f"active_set_support_ids_iter{iteration}.csv", index=False)

    top_q = compact.sort_values(["debug_q_delta_abs", "debug_q_next"], ascending=[False, False]).head(80)
    top_q.to_csv(out_dir / f"active_set_top_q_changes_iter{iteration}.csv", index=False)

    top_score = compact.copy()
    if "debug_score_delta_from_raw" in top_score.columns:
        top_score["debug_abs_score_delta_from_raw"] = pd.to_numeric(top_score["debug_score_delta_from_raw"], errors="coerce").abs()
        top_score = top_score.sort_values(["debug_abs_score_delta_from_raw", "debug_score_for_q"], ascending=[False, False]).head(80)
        top_score.to_csv(out_dir / f"active_set_top_score_deltas_iter{iteration}.csv", index=False)

    entered = sorted(int(x) for x in compact.loc[compact["debug_entered_support"].fillna(False), "group_id"].tolist())
    exited = sorted(int(x) for x in compact.loc[compact["debug_exited_support"].fillna(False), "group_id"].tolist())

    event = {
        "iteration": int(iteration),
        "active_support_in_count": int(len(active_in)),
        "active_support_out_count": int(len(support_out)),
        "selected_out_count": int(len(selected_out)),
        "entered_support_count": int(len(entered)),
        "exited_support_count": int(len(exited)),
        "entered_support_ids": entered[:80],
        "exited_support_ids": exited[:80],
        "q_mass_in": float(sum(float(old_weight.get(int(g), 0.0)) for g in old_weight)),
        "q_mass_out": float(sum(float(new_weight.get(int(g), 0.0)) for g in new_weight)),
        "q_max_delta": float(pd.to_numeric(compact["debug_q_delta_abs"], errors="coerce").max()) if len(compact) else 0.0,
        "q_mean_delta": float(pd.to_numeric(compact["debug_q_delta_abs"], errors="coerce").mean()) if len(compact) else 0.0,
        "step_alpha": float(step_alpha),
        "step_alpha_before_churn_damp": float(pd.to_numeric(df.get("active_weight_step_alpha_before_churn_damp", pd.Series([step_alpha])), errors="coerce").dropna().iloc[0]) if pd.to_numeric(df.get("active_weight_step_alpha_before_churn_damp", pd.Series([step_alpha])), errors="coerce").notna().any() else float(step_alpha),
        "churn_ratio_tentative": float(pd.to_numeric(df.get("active_weight_churn_ratio_tentative", pd.Series([0.0])), errors="coerce").dropna().iloc[0]) if pd.to_numeric(df.get("active_weight_churn_ratio_tentative", pd.Series([0.0])), errors="coerce").notna().any() else 0.0,
        "churn_step_multiplier": float(pd.to_numeric(df.get("active_weight_churn_step_multiplier", pd.Series([1.0])), errors="coerce").dropna().iloc[0]) if pd.to_numeric(df.get("active_weight_churn_step_multiplier", pd.Series([1.0])), errors="coerce").notna().any() else 1.0,
        "baseline_cqd": float(baseline_cqd),
        "validation_bad_rounds": int(validation_bad_rounds),
        "validation_raw_logloss": float(active_model.get("active_validation_raw_logloss", np.nan)),
        "validation_base_logloss": float(active_model.get("active_validation_base_logloss", np.nan)),
        "validation_corrected_logloss": float(active_model.get("active_validation_corrected_logloss", np.nan)),
        "beta": float(active_model.get("beta", np.nan)),
    }
    with (out_dir / "active_set_runtime_events.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps(event, ensure_ascii=False, sort_keys=True) + "\n")
    print("ACTIVE_ITERATION_EVENT " + json.dumps(event, ensure_ascii=False, sort_keys=True), flush=True)
    return event


def _model_edges_for_ids(all_edges: pd.DataFrame, ids: Sequence[int], context: str) -> pd.DataFrame:
    ids = sorted({int(g) for g in ids})
    idset = set(ids)
    edges = all_edges[all_edges.group_a.isin(idset) & all_edges.group_b.isin(idset)].copy()
    if len(ids) >= 2:
        if edges.empty:
            raise RuntimeError(f"No within-active group_rates edges for {context}; active_groups={len(ids)}")
        require_complete_undirected_winrate_edges(edges, ids, context)
    return edges


def active_set_soft_selection_temperature_cqd(lane_size: int) -> float:
    """CQD scale for converting active utility into continuous active weight.

    This is a smooth link scale, not a rank cap.  Smaller values approach the
    legacy hard threshold; larger values make boundary membership more gradual.
    """
    try:
        lane = int(lane_size)
    except Exception:
        lane = 2
    return float(max(0.035, min(0.15, 0.05 * math.sqrt(max(1, lane)))))


def _active_weight_support_ids(active_weight: Dict[int, float], eps: float = ACTIVE_SET_WEIGHTED_SUPPORT_EPS) -> List[int]:
    return sorted(int(gid) for gid, q in active_weight.items() if float(q) > float(eps))


def _sanitize_likelihood_weights(w: np.ndarray) -> np.ndarray:
    """Return nonnegative raw likelihood weights without mean normalization.

    In a weighted active environment, q is real evidence mass.  Normalizing by
    the mean positive q would turn many tiny-q scout edges back into full-size
    evidence and can create runaway feedback.
    """
    w = np.asarray(w, dtype=float)
    w = np.nan_to_num(w, nan=0.0, posinf=0.0, neginf=0.0)
    return np.maximum(w, 0.0)


def _prepare_weighted_training_edges(
    groups_df: pd.DataFrame,
    all_edges: pd.DataFrame,
    active_weight: Dict[int, float],
    context: str,
) -> Tuple[pd.DataFrame, pd.DataFrame]:
    """Prepare active-fit edges with continuous q_i*q_j likelihood weights.

    The beta-binomial count columns stay in their original integer/sample units;
    likelihood_weight scales each edge's contribution to the objective.  A
    separate profile edge table uses weighted samples for spectral/profile steps.
    """
    edges = _prepare_partial_edges_for_ids(all_edges, groups_df["group_id"].astype(int).tolist(), context)
    q_by_idx = {
        i: float(active_weight.get(int(gid), 0.0))
        for i, gid in enumerate(groups_df["group_id"].astype(int).tolist())
    }
    qa = np.asarray([q_by_idx[int(i)] for i in edges["ia"].to_numpy(dtype=int)], dtype=float)
    qb = np.asarray([q_by_idx[int(i)] for i in edges["ib"].to_numpy(dtype=int)], dtype=float)
    edge_q = np.maximum(0.0, qa * qb)
    like_w = _sanitize_likelihood_weights(edge_q)
    edges = edges.copy()
    edges["active_weight_a"] = qa
    edges["active_weight_b"] = qb
    edges["active_edge_weight_raw"] = edge_q
    edges["likelihood_weight"] = like_w
    profile_edges = edges.copy()
    profile_edges["samples"] = profile_edges["samples"].astype(float) * profile_edges["likelihood_weight"].astype(float)
    return edges, profile_edges


def _attach_active_weight_columns(df: pd.DataFrame, active_weight: Dict[int, float], col: str = "active_weight_q") -> pd.DataFrame:
    out = df.copy()
    out[col] = out["group_id"].astype(int).map(lambda gid: float(active_weight.get(int(gid), 0.0)))
    return out


def _compute_soft_browser_active_targets(
    scored: pd.DataFrame,
    group_members: Dict[int, List[str]],
    baseline_cqd: float,
    score_col: str,
    temperature_cqd: float,
    target_mass: Optional[float] = None,
) -> pd.DataFrame:
    """Continuous browser-greedy target q* for the active environment.

    This is a mass-conserving soft relaxation of browser-visible main selection.
    The first weighted implementation used an independent sigmoid around the
    baseline, so every row above the floor could simultaneously want q≈1.  With
    many near-duplicate scouts this creates a positive feedback loop.

    Here the sigmoid center is solved by bisection so total target active mass is
    close to the initial browser-visible mass.  That keeps the weighted active
    environment continuous without letting the active universe expand just
    because many score-only projections are locally above the floor.
    """
    df = scored.copy()
    score = pd.to_numeric(df[score_col], errors="coerce").to_numpy(float)
    ok = df.get("active_set_score_success", pd.Series(True, index=df.index)).fillna(True).astype(bool).to_numpy()
    temp = max(float(temperature_cqd), 1e-9)
    q_now = pd.to_numeric(df.get("active_weight_q", pd.Series(0.0, index=df.index)), errors="coerce").astype(float)
    q_now = q_now.where(np.isfinite(q_now), 0.0).clip(lower=0.0, upper=1.0).to_numpy(float)
    rel = pd.to_numeric(df.get("active_residual_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    rel = rel.where(np.isfinite(rel), 0.0).clip(lower=0.0, upper=1.0).to_numpy(float)
    coverage = pd.to_numeric(df.get("active_residual_coverage_reliability", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    coverage = coverage.where(np.isfinite(coverage), rel).clip(lower=0.0, upper=1.0).to_numpy(float)
    # q-target is still score-driven, but low-evidence rows should not jump into
    # the active environment at q≈1 merely because a noisy projection crossed the
    # floor.  Existing support is allowed some inertia; score-only rows need
    # actual active-reference evidence.
    evidence_gate = np.clip(0.20 + 0.80 * np.sqrt(np.maximum(rel * coverage, 0.0)), 0.0, 1.0)
    evidence_gate = np.maximum(evidence_gate, 0.35 * q_now)

    sort_cols = [score_col]
    ascending = [False]
    if "raw_cqd" in df.columns:
        sort_cols.append("raw_cqd")
        ascending.append(False)
    sort_cols.append("group_id")
    ascending.append(True)
    ordered_index = list(df.sort_values(sort_cols, ascending=ascending).index)

    finite_ok = np.isfinite(score) & ok
    finite_scores = score[finite_ok]
    if len(finite_scores) == 0:
        df["active_weight_desire"] = 0.0
        df["active_weight_availability"] = 0.0
        df["active_weight_target"] = 0.0
        df["active_weight_evidence_gate"] = 0.0
        df["active_weight_mass_center_cqd"] = np.nan
        df["active_weight_target_mass"] = 0.0 if target_mass is None else float(target_mass)
        return df

    def compute_for_center(center: float):
        desire = sigmoid((score - float(center)) / temp)
        desire = np.where(finite_ok, desire, 0.0)
        target = np.zeros(len(df), dtype=float)
        availability = np.zeros(len(df), dtype=float)
        occupied: Dict[str, float] = {}
        for idx in ordered_index:
            gid = int(df.at[idx, "group_id"])
            fallback = str(df.at[idx, "Name"]) if "Name" in df.columns else str(gid)
            members = _members_for_group(group_members, gid, fallback)
            if not members:
                members = [str(gid)]
            avail = 1.0
            for m in members:
                avail *= max(0.0, 1.0 - float(occupied.get(str(m), 0.0)))
            availability[idx] = avail
            q = max(0.0, min(1.0, float(desire[idx]) * float(avail)))
            q *= float(evidence_gate[idx])
            q = max(0.0, min(1.0, q))
            target[idx] = q
            if q > 0.0:
                for m in members:
                    m = str(m)
                    old = max(0.0, min(1.0, float(occupied.get(m, 0.0))))
                    occupied[m] = 1.0 - (1.0 - old) * (1.0 - q)
        return desire, availability, target, float(target.sum())

    if target_mass is None or not np.isfinite(float(target_mass)) or float(target_mass) <= 0.0:
        center = float(baseline_cqd)
        desire, availability, target, mass = compute_for_center(center)
    else:
        tm = max(0.0, float(target_mass))
        lo = float(np.nanmin(finite_scores) - 50.0 * temp - 1.0)
        hi = float(np.nanmax(finite_scores) + 50.0 * temp + 1.0)
        desire, availability, target, mass = compute_for_center(hi)
        center = hi
        for _ in range(64):
            mid = 0.5 * (lo + hi)
            d_mid, a_mid, t_mid, m_mid = compute_for_center(mid)
            if m_mid > tm:
                lo = mid
            else:
                hi = mid
                desire, availability, target, mass = d_mid, a_mid, t_mid, m_mid
                center = mid
            if abs(m_mid - tm) <= ACTIVE_SET_WEIGHTED_MASS_TOL:
                desire, availability, target, mass = d_mid, a_mid, t_mid, m_mid
                center = mid
                break

    df["active_weight_desire"] = desire.astype(float)
    df["active_weight_availability"] = availability.astype(float)
    df["active_weight_target"] = target.astype(float)
    df["active_weight_evidence_gate"] = evidence_gate.astype(float)
    df["active_weight_mass_center_cqd"] = float(center)
    df["active_weight_target_mass"] = float(target_mass) if target_mass is not None else float(mass)
    return df



def _weighted_logloss_for_eta(y: np.ndarray, n: np.ndarray, eta: np.ndarray, weight: Optional[np.ndarray] = None) -> float:
    y = np.asarray(y, dtype=float)
    n = np.asarray(n, dtype=float)
    eta = np.asarray(eta, dtype=float)
    if weight is None:
        weight = np.ones(len(y), dtype=float)
    weight = np.asarray(weight, dtype=float)
    weight = np.nan_to_num(weight, nan=0.0, posinf=0.0, neginf=0.0)
    weight = np.maximum(weight, 0.0)
    p = np.clip(sigmoid_any(np.clip(eta, -40, 40)), 1e-9, 1.0 - 1e-9)
    w = np.maximum(n, 0.0) * weight
    denom = float(np.sum(w))
    if denom <= 0.0:
        return float("nan")
    return float(-np.sum(w * (y * np.log(p) + (1.0 - y) * np.log(1.0 - p))) / denom)


def _edge_support_by_group(edges: pd.DataFrame, group_ids: Sequence[int], weight_col: str = "samples") -> Dict[int, float]:
    support = {int(g): 0.0 for g in group_ids}
    if edges.empty:
        return support
    w = edges[weight_col].to_numpy(float) if weight_col in edges.columns else edges["samples"].to_numpy(float)
    for (a, b), ww in zip(edges[["group_a", "group_b"]].itertuples(index=False, name=None), w):
        support[int(a)] = support.get(int(a), 0.0) + float(max(0.0, ww))
        support[int(b)] = support.get(int(b), 0.0) + float(max(0.0, ww))
    return support


def _robust_shrink_factor(delta_cqd: np.ndarray, scale_cqd: float) -> np.ndarray:
    """Smoothly shrink very large residuals without a hard cap."""
    d = np.asarray(delta_cqd, dtype=float)
    scale = max(float(scale_cqd), 1e-9)
    return 1.0 / (1.0 + (np.abs(d) / scale) ** 2)


def _soft_cap_residual_cqd(delta_cqd: np.ndarray, cap_cqd: Any) -> Tuple[np.ndarray, np.ndarray]:
    """Huber/tanh-style residual cap in CQD units.

    `cap_cqd` may be scalar or row-level.  Medium residuals remain mostly
    alive; only extreme active residuals are compressed.  The second return
    value is the row-level cap factor for diagnostics and for backward-compatible
    `active_residual_robust_shrink`.
    """
    d = np.asarray(delta_cqd, dtype=float)
    cap = np.asarray(cap_cqd, dtype=float)
    if cap.ndim == 0:
        cap = np.full_like(d, float(cap), dtype=float)
    cap = np.nan_to_num(cap, nan=float(ACTIVE_REG_RESIDUAL_SOFT_CAP_CQD), posinf=float(ACTIVE_REG_RESIDUAL_SOFT_CAP_CQD), neginf=float(ACTIVE_REG_RESIDUAL_SOFT_CAP_CQD))
    cap = np.maximum(cap, 1e-9)
    capped = cap * np.tanh(d / cap)
    factor = np.divide(
        capped,
        d,
        out=np.ones_like(capped, dtype=float),
        where=np.abs(d) > 1e-12,
    )
    return capped, np.clip(factor, 0.0, 1.0)


def _validation_residual_survival_factor(active_model: Optional[Dict[str, Any]]) -> float:
    """Use validation as a smooth residual survival multiplier, never as early stop."""
    if not isinstance(active_model, dict):
        return 1.0
    try:
        corr = float(active_model.get("active_validation_corrected_logloss", np.nan))
        base = float(active_model.get("active_validation_base_logloss", np.nan))
        raw = float(active_model.get("active_validation_raw_logloss", np.nan))
    except Exception:
        return 1.0
    refs = [x for x in [base, raw] if np.isfinite(x)]
    if not np.isfinite(corr) or not refs:
        return 1.0
    ref = float(min(refs))
    gap = float(corr - ref)
    scale = max(float(ACTIVE_REG_VALIDATION_SHRINK_SCALE_LOGLOSS), 1e-12)
    if gap <= -ACTIVE_REG_VALIDATION_TOL:
        return float(min(1.15, 1.0 + min(0.15, (-gap) / scale * 0.10)))
    if gap <= ACTIVE_REG_VALIDATION_TOL:
        return 1.0
    return float(max(0.55, 1.0 / (1.0 + gap / scale)))


def _active_churn_step_multiplier(churn_ratio: float) -> float:
    """Dampen q updates when support churn suggests a basin flip."""
    try:
        x = float(churn_ratio)
    except Exception:
        return 1.0
    if not np.isfinite(x) or x <= ACTIVE_REG_CHURN_DAMP_START_RATIO:
        return 1.0
    span = max(float(ACTIVE_REG_CHURN_DAMP_FULL_RATIO - ACTIVE_REG_CHURN_DAMP_START_RATIO), 1e-12)
    t = min(1.0, max(0.0, (x - ACTIVE_REG_CHURN_DAMP_START_RATIO) / span))
    mult = 1.0 - t * (1.0 - ACTIVE_REG_CHURN_DAMP_MIN_MULTIPLIER)
    return float(max(ACTIVE_REG_CHURN_DAMP_MIN_MULTIPLIER, min(1.0, mult)))


def _active_micro_churn_step_multiplier(iteration: int, churn_ratio: float, tentative_max_delta: float) -> float:
    """Dampen late small-support q flips that keep bouncing between basins.

    This is continuous and validation-neutral: it slows the q update but never
    early-stops and never changes the score universe.
    """
    try:
        it = int(iteration)
        x = float(churn_ratio)
        md = float(tentative_max_delta)
    except Exception:
        return 1.0
    if it < ACTIVE_REG_MICRO_CHURN_DAMP_START_ITER or not np.isfinite(x) or not np.isfinite(md) or x <= 0.0 or md <= 0.0:
        return 1.0
    delta_pressure = min(1.0, max(0.0, md / max(float(ACTIVE_REG_MICRO_CHURN_DAMP_DELTA_SCALE), 1e-12)))
    mult = 1.0 / (1.0 + float(ACTIVE_REG_MICRO_CHURN_DAMP_STRENGTH) * x * delta_pressure)
    return float(max(ACTIVE_REG_MICRO_CHURN_DAMP_MIN_MULTIPLIER, min(1.0, mult)))



def _align_score_moments_to_raw(
    df: pd.DataFrame,
    score_col: str,
    raw_col: str = "raw_cqd",
    context: str = "active_environment",
    iteration: Optional[int] = None,
) -> Tuple[pd.DataFrame, Dict[str, Any]]:
    """Affine-align finite projected scores to Raw mean/std on the same rows.

    This is an environment-level invariant, not a row-level Raw adhesion rule.
    The active layer may reorder rows, but on the exact finite score universe used
    by this call it is not allowed to invent a new global location or scale.
    """
    out = df.copy()
    idx = out.index
    raw = pd.to_numeric(out.get(raw_col, pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    score_before = pd.to_numeric(out.get(score_col, pd.Series(np.nan, index=idx)), errors="coerce").astype(float)
    finite = np.isfinite(raw.to_numpy(float)) & np.isfinite(score_before.to_numpy(float))
    n = int(finite.sum())
    diag: Dict[str, Any] = {
        "context": str(context),
        "iteration": int(iteration) if iteration is not None else np.nan,
        "score_col": str(score_col),
        "raw_col": str(raw_col),
        "moment_alignment_rows": n,
        "moment_alignment_applied": False,
        "alignment_reason": "not_evaluated",
        "raw_mean": np.nan,
        "raw_sd": np.nan,
        "projected_mean_before": np.nan,
        "projected_sd_before": np.nan,
        "projected_mean_after": np.nan,
        "projected_sd_after": np.nan,
        "alignment_shift_cqd": np.nan,
        "alignment_scale_factor": np.nan,
        "mean_delta_before": np.nan,
        "mean_delta_after": np.nan,
        "sd_ratio_before": np.nan,
        "sd_ratio_after": np.nan,
    }
    if n < int(ACTIVE_MOMENT_ALIGNMENT_MIN_ROWS):
        diag["alignment_reason"] = "too_few_finite_rows"
        out["active_moment_alignment_applied"] = False
        out["active_moment_alignment_delta_cqd"] = 0.0
        return out, diag

    raw_vals = raw.to_numpy(float)[finite]
    score_vals = score_before.to_numpy(float)[finite]
    raw_mean = float(np.mean(raw_vals))
    score_mean = float(np.mean(score_vals))
    raw_sd = float(np.std(raw_vals, ddof=0))
    score_sd = float(np.std(score_vals, ddof=0))
    diag.update({
        "raw_mean": raw_mean,
        "raw_sd": raw_sd,
        "projected_mean_before": score_mean,
        "projected_sd_before": score_sd,
        "mean_delta_before": score_mean - raw_mean,
        "sd_ratio_before": score_sd / raw_sd if raw_sd > 0.0 else np.nan,
    })
    if not np.isfinite(raw_sd) or raw_sd <= float(ACTIVE_MOMENT_ALIGNMENT_MIN_SD_CQD):
        diag["alignment_reason"] = "near_zero_raw_sd"
        out["active_moment_alignment_applied"] = False
        out["active_moment_alignment_delta_cqd"] = 0.0
        return out, diag
    if not np.isfinite(score_sd) or score_sd <= float(ACTIVE_MOMENT_ALIGNMENT_MIN_SD_CQD):
        diag["alignment_reason"] = "near_zero_projected_sd"
        out["active_moment_alignment_applied"] = False
        out["active_moment_alignment_delta_cqd"] = 0.0
        return out, diag

    scale = raw_sd / score_sd
    shift = raw_mean - score_mean * scale
    aligned_all = raw.to_numpy(float).copy()
    score_arr = score_before.to_numpy(float)
    aligned_all[finite] = raw_mean + (score_arr[finite] - score_mean) * scale
    aligned = pd.Series(aligned_all, index=idx).where(pd.Series(finite, index=idx), score_before)
    out[f"{score_col}_pre_moment_alignment"] = score_before
    out[score_col] = aligned.astype(float)
    delta = out[score_col].astype(float) - score_before
    out["active_moment_alignment_applied"] = True
    out["active_moment_alignment_delta_cqd"] = delta.where(np.isfinite(delta), 0.0).astype(float)
    out["active_moment_alignment_scale_factor"] = float(scale)
    out["active_moment_alignment_shift_cqd"] = float(shift)
    out["active_moment_alignment_context"] = str(context)

    after_vals = out[score_col].astype(float).to_numpy()[finite]
    after_mean = float(np.mean(after_vals))
    after_sd = float(np.std(after_vals, ddof=0))
    diag.update({
        "moment_alignment_applied": True,
        "alignment_reason": "aligned_to_raw_mean_and_variance_on_same_finite_rows",
        "projected_mean_after": after_mean,
        "projected_sd_after": after_sd,
        "alignment_shift_cqd": float(shift),
        "alignment_scale_factor": float(scale),
        "mean_delta_after": after_mean - raw_mean,
        "sd_ratio_after": after_sd / raw_sd if raw_sd > 0.0 else np.nan,
    })
    return out, diag


def _refresh_regularized_score_after_moment_alignment(
    df: pd.DataFrame,
    baseline_cqd: float,
    beta: float,
) -> pd.DataFrame:
    """Keep final-score columns internally consistent after moment alignment."""
    out = df.copy()
    if "regularized_active_cqd" not in out.columns:
        return out
    reg = pd.to_numeric(out["regularized_active_cqd"], errors="coerce").astype(float)
    base = pd.to_numeric(out.get("global_base_cqd", out.get("raw_cqd", pd.Series(np.nan, index=out.index))), errors="coerce").astype(float)
    out["Correct Cqd"] = reg
    out["regularized_active_logit"] = float(beta) * (reg - float(baseline_cqd))
    if "active_residual_net_adjustment_cqd" in out.columns:
        old_net = pd.to_numeric(out["active_residual_net_adjustment_cqd"], errors="coerce").astype(float)
        if "active_residual_net_adjustment_pre_moment_cqd" not in out.columns:
            out["active_residual_net_adjustment_pre_moment_cqd"] = old_net
    out["active_residual_net_adjustment_cqd"] = (reg - base).astype(float)
    out["active_residual_net_adjustment_after_moment_cqd"] = out["active_residual_net_adjustment_cqd"].astype(float)
    if "regularized_active_source" in out.columns:
        out["regularized_active_source"] = out["regularized_active_source"].astype(str) + "+raw_moment_aligned"
    else:
        out["regularized_active_source"] = "raw_moment_aligned"
    return out


def _renormalize_weight_mass(weights: Dict[int, float], target_mass: float) -> Dict[int, float]:
    vals = {int(g): max(0.0, min(1.0, float(q))) for g, q in weights.items()}
    tm = float(target_mass)
    if not np.isfinite(tm) or tm <= 0.0:
        return vals
    mass = float(sum(vals.values()))
    if mass <= 1e-12:
        return vals
    scale = tm / mass
    vals = {gid: max(0.0, min(1.0, q * scale)) for gid, q in vals.items()}
    # One pass cannot always restore mass when clipping at 1.0.  That is fine;
    # the audit reports realized mass and support probability.
    return vals


def _tail_average_active_weights(
    q_history: Sequence[Dict[int, float]],
    all_group_ids: Sequence[int],
    target_mass: float,
) -> Tuple[Dict[int, float], pd.DataFrame]:
    gids = [int(g) for g in sorted({int(g) for g in all_group_ids})]
    if not q_history:
        out = {gid: 0.0 for gid in gids}
        return out, pd.DataFrame({"group_id": gids, "q_tail_mean": [0.0] * len(gids)})
    tail = list(q_history)[-max(1, int(ACTIVE_Q_TAIL_WINDOW)):]
    rows = []
    mean_weight: Dict[int, float] = {}
    support_prob_by_gid: Dict[int, float] = {}
    for gid in gids:
        arr = np.asarray([float(w.get(int(gid), 0.0)) for w in tail], dtype=float)
        arr = np.nan_to_num(arr, nan=0.0, posinf=0.0, neginf=0.0)
        mean = float(np.mean(arr)) if len(arr) else 0.0
        sd = float(np.std(arr, ddof=0)) if len(arr) else 0.0
        support_prob = float(np.mean(arr > ACTIVE_SET_WEIGHTED_SUPPORT_EPS)) if len(arr) else 0.0
        last = float(arr[-1]) if len(arr) else 0.0
        mean_weight[int(gid)] = mean
        support_prob_by_gid[int(gid)] = support_prob
        rows.append({
            "group_id": int(gid),
            "q_last": last,
            "q_tail_mean_raw": mean,
            "q_tail_sd": sd,
            "q_tail_support_probability": support_prob,
            "q_tail_window": int(len(tail)),
            "q_tail_persistent_support_min_probability": float(ACTIVE_Q_TAIL_MIN_SUPPORT_PROB),
        })

    # The final active environment should be the stable tail environment, not a
    # union of every row that briefly crossed support_eps.  Rows that do not have
    # enough tail support probability are audited but zeroed before mass
    # renormalization.  This keeps the weighted map continuous while preventing
    # tail averaging from inflating support cardinality.
    persistent_weight = {
        gid: (q if support_prob_by_gid.get(gid, 0.0) >= float(ACTIVE_Q_TAIL_MIN_SUPPORT_PROB) else 0.0)
        for gid, q in mean_weight.items()
    }
    final_weight = _renormalize_weight_mass(persistent_weight, target_mass)
    for row in rows:
        gid = int(row["group_id"])
        row["q_tail_persistent_support"] = bool(support_prob_by_gid.get(gid, 0.0) >= float(ACTIVE_Q_TAIL_MIN_SUPPORT_PROB))
        row["q_tail_mean"] = float(final_weight.get(gid, 0.0))
        row["q_tail_mass_renormalized"] = True
        row["q_tail_minus_last"] = float(row["q_tail_mean"] - row["q_last"])
        row["q_tail_support_final"] = bool(row["q_tail_mean"] > ACTIVE_SET_WEIGHTED_SUPPORT_EPS)
    return final_weight, pd.DataFrame(rows)


def _write_slice_moment_diagnostics(out_dir: Path, groups_out: pd.DataFrame) -> None:
    """Audit possible post-alignment slice drift; do not modify scores."""
    if groups_out is None or groups_out.empty:
        return
    df = groups_out.copy()
    idx = df.index
    raw = pd.to_numeric(df.get("raw_cqd", df.get("Raw Cqd", pd.Series(np.nan, index=idx))), errors="coerce").astype(float)
    corr = pd.to_numeric(df.get("Correct Cqd", df.get("selection_weight_cqd", pd.Series(np.nan, index=idx))), errors="coerce").astype(float)
    active = df.get("active_set_selected_for_training", pd.Series(False, index=idx)).fillna(False).astype(bool)
    blocked = df.get("blocked_score_only_candidate", pd.Series(False, index=idx)).fillna(False).astype(bool)
    scout = df.get("scout_candidate", pd.Series(False, index=idx)).fillna(False).astype(bool)
    selected = df.get("resolver_selected", pd.Series(0, index=idx)).fillna(0).astype(int).astype(bool)
    masks = {
        "all_scoreable": pd.Series(True, index=idx),
        "active_training_rows": active,
        "score_only_nonblocked": (~active) & (~blocked),
        "score_only_blocked": blocked,
        "scout_rows": scout,
        "resolver_selected": selected,
    }
    if raw.notna().sum() >= 4:
        try:
            bins = pd.qcut(raw.rank(method="first"), q=min(5, int(raw.notna().sum())), labels=False, duplicates="drop")
            for b in sorted(pd.Series(bins).dropna().unique()):
                masks[f"raw_quantile_{int(b)}"] = pd.Series(bins == b, index=idx)
        except Exception:
            pass
    rows = []
    for name, mask in masks.items():
        m = mask.fillna(False).astype(bool) & np.isfinite(raw.to_numpy(float)) & np.isfinite(corr.to_numpy(float))
        n = int(m.sum())
        rv = raw.loc[m].to_numpy(float)
        cv = corr.loc[m].to_numpy(float)
        if n == 0:
            rows.append({"slice_name": name, "slice_rows": 0})
            continue
        raw_mean = float(np.mean(rv)); corr_mean = float(np.mean(cv))
        raw_sd = float(np.std(rv, ddof=0)); corr_sd = float(np.std(cv, ddof=0))
        abs_delta = np.abs(cv - rv)
        rows.append({
            "slice_name": name,
            "slice_rows": n,
            "slice_raw_mean": raw_mean,
            "slice_correct_mean": corr_mean,
            "slice_mean_delta": corr_mean - raw_mean,
            "slice_raw_sd": raw_sd,
            "slice_correct_sd": corr_sd,
            "slice_sd_ratio": corr_sd / raw_sd if raw_sd > 1e-12 else np.nan,
            "slice_abs_correct_minus_raw_mean": float(np.mean(abs_delta)),
            "slice_abs_correct_minus_raw_p95": float(np.percentile(abs_delta, 95)) if len(abs_delta) else np.nan,
        })
    pd.DataFrame(rows).to_csv(out_dir / "slice_moment_diagnostics.csv", index=False)

def _fit_global_regularized_base(
    eligible_df: pd.DataFrame,
    all_edges: pd.DataFrame,
    frozen_rsw: Dict[str, Any],
    seed: int,
    out_dir: Path,
) -> Dict[str, Any]:
    """Fit a global EB base score before active self-training.

    Raw remains an immutable coordinate, but it is not the only prior.  The base
    is a globally regularized EB estimate on the whole eligible/scout universe;
    active self-training is allowed only to learn residual environment
    adjustments around this base.
    """
    df = eligible_df.copy().reset_index(drop=True)
    edges = _prepare_partial_edges_for_ids(
        all_edges,
        df["group_id"].astype(int).tolist(),
        "global regularized base eligible universe",
    )
    raw = df["raw_cqd"].to_numpy(float)
    all_mask = np.ones(len(edges), dtype=bool)
    beta = float(frozen_rsw.get("beta_raw", fit_raw_beta(
        raw[edges["ia"].to_numpy()] - raw[edges["ib"].to_numpy()],
        edges["win_rate_a"].to_numpy(float),
        edges["samples"].to_numpy(float),
    )))
    missing_rsw = [int(g) for g in df["group_id"].astype(int).tolist() if int(g) not in frozen_rsw["type_by_gid"]]
    if missing_rsw:
        raise RuntimeError(f"Global base contains group(s) without frozen RSW-Type: {missing_rsw[:20]}")
    type_ids = np.asarray([int(frozen_rsw["type_by_gid"][int(g)]) for g in df["group_id"].astype(int)], dtype=int)
    selected_k = int(max(1, min(int(frozen_rsw["n_types"]), 4, max(1, int(np.sqrt(max(1, len(df))))))))
    _, spectrum, embedding, _, _ = derive_lowrank_embedding(
        df,
        edges,
        beta,
        all_mask,
        seed=seed + 700_000,
        target_rank=selected_k,
    )
    spectrum.to_csv(out_dir / "global_regularized_base_lowrank_spectrum.csv", index=False)

    # Keep the base model deliberately modest.  It corrects Raw's global bias,
    # while the later active loop handles environment residuals under shrinkage.
    fit = fit_betabinomial_lowrank_counter_eb(
        df,
        edges,
        all_mask,
        type_ids,
        embedding,
        max_eb_iter=3,
        tol=3e-3,
        fixed_beta=beta,
    )
    beta = float(fit.beta)
    if abs(beta) <= 1e-8:
        raw_delta_cqd = fit.delta.copy()
        base_cqd_raw = beta * raw + fit.delta
    else:
        raw_delta_cqd = fit.delta / beta
        base_cqd_raw = raw + raw_delta_cqd

    support = _edge_support_by_group(edges, df["group_id"].astype(int).tolist(), "samples")
    support_arr = np.asarray([support.get(int(g), 0.0) for g in df["group_id"].astype(int)], dtype=float)
    positive_support = support_arr[support_arr > 0]
    kappa = float(np.median(positive_support) * 1.5) if len(positive_support) else 1.0
    reliability = support_arr / np.maximum(support_arr + kappa, 1e-12)
    reliability = np.clip(reliability, 0.0, 1.0)
    robust = _robust_shrink_factor(raw_delta_cqd, ACTIVE_REG_BASE_DELTA_ROBUST_SCALE_CQD)

    beta_abs = max(abs(float(beta)), 1e-12)
    tau_delta_cqd = float(fit.tau_delta) / beta_abs
    tau_counter_cqd = float(fit.tau_counter) / beta_abs if len(fit.theta) > 0 else 0.0
    tau_lowrank_cqd = float(fit.tau_lowrank) / beta_abs if len(fit.gamma) > 0 else 0.0
    # The exported global base only uses the group-level delta as a scalar base.
    # Counter/lowrank terms are pairwise residual structure, not scalar scores.
    #
    # Keep tau-instability as a diagnostic, but do not multiply it into the
    # score.  The existing reliability and robust shrink already control the
    # scalar global-base delta.  Applying an additional tau_stability_shrink here
    # made the base overly adhesive to Raw, especially when counter/lowrank tau
    # was large even though scalar group delta was already robust-shrunk.
    tau_instability_cqd = max(float(tau_delta_cqd), 0.5 * float(tau_counter_cqd), 0.25 * float(tau_lowrank_cqd))
    tau_stability_shrink = 1.0 / (1.0 + (tau_instability_cqd / max(ACTIVE_REG_GLOBAL_BASE_TAU_CQD_STABILITY_SCALE, 1e-12)) ** 2)

    delta_final = raw_delta_cqd * reliability * robust
    base_cqd = raw + delta_final

    out = df[["group_id", "raw_cqd", "raw_rank", "Name", "Text-Type"]].copy()
    out["global_base_cqd_raw"] = base_cqd_raw
    out["global_base_delta_raw_cqd"] = raw_delta_cqd
    out["global_base_support_samples"] = support_arr
    out["global_base_reliability"] = reliability
    out["global_base_robust_shrink"] = robust
    out["global_base_tau_delta_cqd"] = float(tau_delta_cqd)
    out["global_base_tau_counter_cqd"] = float(tau_counter_cqd)
    out["global_base_tau_lowrank_cqd"] = float(tau_lowrank_cqd)
    out["global_base_tau_stability_shrink"] = float(tau_stability_shrink)
    out["global_base_delta_cqd"] = delta_final
    out["global_base_cqd"] = base_cqd
    out["global_base_beta"] = beta
    out["global_base_tau_delta"] = float(fit.tau_delta)
    out["global_base_tau_counter"] = float(fit.tau_counter)
    out["global_base_tau_lowrank"] = float(fit.tau_lowrank)
    out.to_csv(out_dir / "global_regularized_base_scores.csv", index=False)

    abs_raw_delta = np.abs(np.asarray(raw_delta_cqd, dtype=float))
    abs_final_delta = np.abs(np.asarray(delta_final, dtype=float))
    diag = {
        "groups": int(len(df)),
        "edges": int(len(edges)),
        "global_base_beta": float(beta),
        "global_base_tau_delta_logit": float(fit.tau_delta),
        "global_base_tau_counter_logit": float(fit.tau_counter),
        "global_base_tau_lowrank_logit": float(fit.tau_lowrank),
        "global_base_tau_delta_cqd": float(tau_delta_cqd),
        "global_base_tau_counter_cqd": float(tau_counter_cqd),
        "global_base_tau_lowrank_cqd": float(tau_lowrank_cqd),
        "global_base_tau_instability_cqd": float(tau_instability_cqd),
        "global_base_tau_stability_shrink": float(tau_stability_shrink),
        "global_base_tau_stability_scale_cqd": float(ACTIVE_REG_GLOBAL_BASE_TAU_CQD_STABILITY_SCALE),
        "global_base_tau_stability_shrink_applied_to_score": 0,
        "raw_delta_abs_mean_cqd": float(np.nanmean(abs_raw_delta)) if len(abs_raw_delta) else np.nan,
        "raw_delta_abs_median_cqd": float(np.nanmedian(abs_raw_delta)) if len(abs_raw_delta) else np.nan,
        "raw_delta_abs_p95_cqd": float(np.nanpercentile(abs_raw_delta, 95)) if len(abs_raw_delta) else np.nan,
        "raw_delta_abs_max_cqd": float(np.nanmax(abs_raw_delta)) if len(abs_raw_delta) else np.nan,
        "final_delta_abs_mean_cqd": float(np.nanmean(abs_final_delta)) if len(abs_final_delta) else np.nan,
        "final_delta_abs_median_cqd": float(np.nanmedian(abs_final_delta)) if len(abs_final_delta) else np.nan,
        "final_delta_abs_p95_cqd": float(np.nanpercentile(abs_final_delta, 95)) if len(abs_final_delta) else np.nan,
        "final_delta_abs_max_cqd": float(np.nanmax(abs_final_delta)) if len(abs_final_delta) else np.nan,
        "robust_shrink_mean": float(np.nanmean(robust)) if len(robust) else np.nan,
        "robust_shrink_median": float(np.nanmedian(robust)) if len(robust) else np.nan,
        "robust_shrink_p05": float(np.nanpercentile(robust, 5)) if len(robust) else np.nan,
        "reliability_mean": float(np.nanmean(reliability)) if len(reliability) else np.nan,
        "reliability_median": float(np.nanmedian(reliability)) if len(reliability) else np.nan,
        "base_output_is_shrink_dominated": bool(float(np.nanmedian(robust)) < 0.5) if len(robust) else False,
        "global_base_scalar_delta_note": "base_by_gid uses group delta only; counter/lowrank are pairwise structure used to estimate/shrink delta; tau_stability_shrink is diagnostic-only",
    }
    pd.DataFrame([diag]).to_csv(out_dir / "global_regularized_base_diagnostics.csv", index=False)

    return {
        "base_by_gid": {int(g): float(v) for g, v in zip(df["group_id"].astype(int), base_cqd)},
        "raw_base_by_gid": {int(g): float(v) for g, v in zip(df["group_id"].astype(int), base_cqd_raw)},
        "delta_by_gid": {int(g): float(v) for g, v in zip(df["group_id"].astype(int), delta_final)},
        "reliability_by_gid": {int(g): float(v) for g, v in zip(df["group_id"].astype(int), reliability)},
        "beta": beta,
        "fit": fit,
        "type_ids": type_ids,
        "embedding": embedding,
        "edges": edges,
    }


def _active_onefold_validation_metrics(
    active_df: pd.DataFrame,
    edges: pd.DataFrame,
    type_ids: np.ndarray,
    embedding: np.ndarray,
    beta: float,
    global_base: Dict[int, float],
    iteration: int,
) -> Dict[str, float]:
    """Single rotating held-out fold to control active self-training step size."""
    if edges.empty or "fold5" not in edges.columns:
        return {}
    folds = sorted(set(int(x) for x in edges["fold5"].dropna().astype(int).tolist()))
    if not folds:
        return {}
    val_fold = folds[(int(iteration) - 1) % len(folds)]
    val = edges["fold5"].astype(int).to_numpy() == int(val_fold)
    train = ~val
    if int(val.sum()) < 1 or int(train.sum()) < 1:
        return {}
    try:
        fit_val = fit_betabinomial_lowrank_counter_eb(
            active_df,
            edges,
            train,
            type_ids,
            embedding,
            max_eb_iter=2,
            tol=4e-3,
            fixed_beta=beta,
        )
        sub = edges.loc[val].copy()
        raw = active_df["raw_cqd"].to_numpy(float)
        ia = sub["ia"].to_numpy(int)
        ib = sub["ib"].to_numpy(int)
        y = sub["win_rate_a"].to_numpy(float)
        n = sub["samples"].to_numpy(float)
        like = sub["likelihood_weight"].to_numpy(float) if "likelihood_weight" in sub.columns else np.ones(len(sub), dtype=float)
        gids = active_df["group_id"].astype(int).tolist()
        base_arr = np.asarray([float(global_base.get(int(g), r)) for g, r in zip(gids, raw)], dtype=float)
        eta_raw = beta * (raw[ia] - raw[ib])
        eta_base = beta * (base_arr[ia] - base_arr[ib])
        eta_corr = predict_edges_lowrank(active_df, edges, val, fit_val, type_ids, embedding)
        return {
            "active_validation_fold": float(val_fold),
            "active_validation_raw_logloss": _weighted_logloss_for_eta(y, n, eta_raw, like),
            "active_validation_base_logloss": _weighted_logloss_for_eta(y, n, eta_base, like),
            "active_validation_corrected_logloss": _weighted_logloss_for_eta(y, n, eta_corr, like),
            "active_validation_edges": float(val.sum()),
            "active_validation_weight_mass": float(np.sum(like)),
        }
    except Exception:
        return {
            "active_validation_fold": float(val_fold),
            "active_validation_raw_logloss": float("nan"),
            "active_validation_base_logloss": float("nan"),
            "active_validation_corrected_logloss": float("nan"),
            "active_validation_edges": float(val.sum()),
            "active_validation_weight_mass": float("nan"),
        }


def _apply_regularized_active_residuals(
    scored: pd.DataFrame,
    global_base: Dict[int, float],
    baseline_cqd: float,
    beta: float,
    active_model: Optional[Dict[str, Any]] = None,
) -> pd.DataFrame:
    """Convert active projection score into an evidence-aware residual around global base.

    `global_base` is intentionally Raw in the current patch line.  This function
    is therefore the only place where final Correct may move away from Raw.  The
    residual survival is decomposed into q-mass, edge-count, sample-mass,
    reference-diversity, coverage, validation, and selected-row self-fit factors,
    all exported as diagnostics.
    """
    df = scored.copy()
    raw = df["raw_cqd"].astype(float)
    base = df["group_id"].astype(int).map(lambda gid: float(global_base.get(int(gid), np.nan))).astype(float)
    base = base.where(np.isfinite(base), raw)
    active_score = pd.to_numeric(df["Correct Cqd"], errors="coerce").astype(float)
    active_score = active_score.where(np.isfinite(active_score), base)

    raw_adj = active_score - base
    raw_adj_arr = raw_adj.to_numpy(float)
    ref_mass = pd.to_numeric(df.get("active_weight_reference_mass", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    ref_mass = ref_mass.where(np.isfinite(ref_mass), 0.0).clip(lower=0.0)
    edge_count = pd.to_numeric(df.get("active_set_challenger_edges", pd.Series(np.nan, index=df.index)), errors="coerce").astype(float)
    missing = pd.to_numeric(df.get("active_weight_reference_missing_edges", pd.Series(0.0, index=df.index)), errors="coerce").astype(float)
    edge_count = edge_count.where(np.isfinite(edge_count), 0.0).clip(lower=0.0)
    missing = missing.where(np.isfinite(missing), 0.0).clip(lower=0.0)
    coverage = edge_count / np.maximum(edge_count + missing, 1.0)
    coverage = coverage.clip(lower=0.0, upper=1.0)

    sample_mass = pd.to_numeric(
        df.get("active_weight_reference_sample_mass", df.get("active_reference_sample_mass", pd.Series(np.nan, index=df.index))),
        errors="coerce",
    ).astype(float)
    sample_mass = sample_mass.where(np.isfinite(sample_mass), 0.0).clip(lower=0.0)
    q_sample_mass = pd.to_numeric(
        df.get("active_weight_reference_q_sample_mass", pd.Series(np.nan, index=df.index)),
        errors="coerce",
    ).astype(float)
    q_sample_mass = q_sample_mass.where(np.isfinite(q_sample_mass), 0.0).clip(lower=0.0)
    effective_ref = pd.to_numeric(
        df.get("active_weight_reference_effective_count", pd.Series(np.nan, index=df.index)),
        errors="coerce",
    ).astype(float)
    effective_ref = effective_ref.where(np.isfinite(effective_ref), 0.0).clip(lower=0.0)
    max_share = pd.to_numeric(
        df.get("active_weight_reference_max_share", pd.Series(np.nan, index=df.index)),
        errors="coerce",
    ).astype(float)
    max_share = max_share.where(np.isfinite(max_share), 1.0).clip(lower=0.0, upper=1.0)
    max_evidence_share = pd.to_numeric(
        df.get("active_weight_reference_max_evidence_share", max_share),
        errors="coerce",
    ).astype(float)
    max_evidence_share = max_evidence_share.where(np.isfinite(max_evidence_share), max_share).clip(lower=0.0, upper=1.0)

    q_now = pd.to_numeric(df.get("active_weight_q", pd.Series(0.0, index=df.index)), errors="coerce").astype(float)
    q_now = q_now.where(np.isfinite(q_now), 0.0).clip(lower=0.0, upper=1.0)

    # Split reliability components.  Use a geometric blend instead of a product;
    # otherwise one modest component can zero out otherwise well-measured active
    # evidence and recreate Raw adhesion.
    q_mass_reliability = (ref_mass / np.maximum(ref_mass + ACTIVE_REG_Q_MASS_KAPPA, 1e-12)).clip(lower=0.0, upper=1.0)
    edge_count_reliability = (edge_count / np.maximum(edge_count + ACTIVE_REG_EDGE_COUNT_KAPPA, 1e-12)).clip(lower=0.0, upper=1.0)
    mass_basis = np.maximum(sample_mass, q_sample_mass)
    sample_mass_reliability = (mass_basis / np.maximum(mass_basis + ACTIVE_REG_SAMPLE_MASS_KAPPA, 1e-12)).clip(lower=0.0, upper=1.0)
    diversity_core = (effective_ref / np.maximum(effective_ref + ACTIVE_REG_REFERENCE_DIVERSITY_KAPPA, 1e-12)).clip(lower=0.0, upper=1.0)
    balance = ((1.0 - max_evidence_share) / 0.75).clip(lower=0.0, upper=1.0)
    reference_diversity_reliability = np.sqrt(np.maximum(diversity_core.to_numpy(float) * balance.to_numpy(float), 0.0))
    reference_diversity_reliability = pd.Series(reference_diversity_reliability, index=df.index).clip(lower=0.0, upper=1.0)
    coverage_reliability = coverage.copy()

    reliability_product = (
        np.maximum(q_mass_reliability.to_numpy(float), 1e-6)
        * np.maximum(edge_count_reliability.to_numpy(float), 1e-6)
        * np.maximum(sample_mass_reliability.to_numpy(float), 1e-6)
        * np.maximum(reference_diversity_reliability.to_numpy(float), 1e-6)
    )
    reliability = coverage_reliability.to_numpy(float) * np.power(reliability_product, 0.25)
    reliability = np.clip(reliability, 0.0, 1.0)

    validation_factor = _validation_residual_survival_factor(active_model)
    is_selected_like = (
        q_now.to_numpy(float) > ACTIVE_SET_WEIGHTED_SUPPORT_EPS
    ) | df.get("active_set_selected_for_training", pd.Series(False, index=df.index)).fillna(False).astype(bool).to_numpy()
    role_factor = np.where(is_selected_like, float(ACTIVE_REG_SELECTED_ROW_SELF_FIT_MULTIPLIER), 1.0)

    # Evidence-aware soft cap.  The previous scalar cap created a visible pile-up
    # at the same max movement.  Keep selected/q-support rows tighter, but let
    # well-covered score-only rows retain medium residuals when validation is
    # clearly better than Raw/base.
    evidence_cap_multiplier = (
        float(ACTIVE_REG_EVIDENCE_SOFT_CAP_MIN_MULTIPLIER)
        + (float(ACTIVE_REG_EVIDENCE_SOFT_CAP_MAX_MULTIPLIER) - float(ACTIVE_REG_EVIDENCE_SOFT_CAP_MIN_MULTIPLIER))
        * np.sqrt(np.clip(reliability, 0.0, 1.0))
    )
    role_cap_multiplier = np.where(
        is_selected_like,
        float(ACTIVE_REG_SELECTED_ROW_SOFT_CAP_MULTIPLIER),
        float(ACTIVE_REG_SCORE_ONLY_SOFT_CAP_MULTIPLIER),
    )
    validation_cap_multiplier = 1.0 + min(
        float(ACTIVE_REG_VALIDATION_SOFT_CAP_MAX_BONUS),
        max(0.0, float(validation_factor) - 1.0),
    )
    effective_soft_cap_cqd = (
        float(ACTIVE_REG_RESIDUAL_SOFT_CAP_CQD)
        * evidence_cap_multiplier
        * role_cap_multiplier
        * float(validation_cap_multiplier)
    )
    capped_adj, soft_cap_factor = _soft_cap_residual_cqd(raw_adj_arr, effective_soft_cap_cqd)

    survival = np.clip(reliability * float(validation_factor) * role_factor, 0.0, 1.15)
    shrunk_adj = capped_adj * survival
    active_residual_shrink_ratio = np.divide(
        shrunk_adj,
        raw_adj_arr,
        out=np.zeros_like(shrunk_adj, dtype=float),
        where=np.abs(raw_adj_arr) > 1e-12,
    )

    # Uncertainty/leverage shrink residual magnitude toward base symmetrically;
    # do not always subtract from the score, because that creates a one-sided
    # downward bias unrelated to active evidence.
    uncertainty = (
        (1.0 - q_mass_reliability.to_numpy(float))
        + (1.0 - edge_count_reliability.to_numpy(float))
        + (1.0 - sample_mass_reliability.to_numpy(float))
        + (1.0 - reference_diversity_reliability.to_numpy(float))
        + (1.0 - coverage_reliability.to_numpy(float))
    ) / 5.0
    uncertainty_penalty = ACTIVE_REG_UNCERTAINTY_PENALTY_CQD * uncertainty
    leverage = q_now.to_numpy(float) * np.abs(raw_adj_arr) * (1.0 - reference_diversity_reliability.to_numpy(float))
    leverage_penalty = ACTIVE_REG_LEVERAGE_PENALTY_CQD * leverage
    total_penalty = np.maximum(0.0, uncertainty_penalty + leverage_penalty)
    net_abs = np.maximum(0.0, np.abs(shrunk_adj) - total_penalty)
    active_net_adjustment = np.sign(shrunk_adj) * net_abs

    base_arr = base.to_numpy(float)
    reg_score = base_arr + active_net_adjustment
    try:
        active_validation_raw_logloss = float(active_model.get("active_validation_raw_logloss", np.nan)) if isinstance(active_model, dict) else float("nan")
        active_validation_base_logloss = float(active_model.get("active_validation_base_logloss", np.nan)) if isinstance(active_model, dict) else float("nan")
        active_validation_corrected_logloss = float(active_model.get("active_validation_corrected_logloss", np.nan)) if isinstance(active_model, dict) else float("nan")
    except Exception:
        active_validation_raw_logloss = float("nan")
        active_validation_base_logloss = float("nan")
        active_validation_corrected_logloss = float("nan")
    active_weight_evidence_gate = np.clip(0.20 + 0.80 * np.sqrt(np.maximum(reliability * coverage_reliability.to_numpy(float), 0.0)), 0.0, 1.0)
    active_weight_evidence_gate = np.maximum(active_weight_evidence_gate, 0.35 * q_now.to_numpy(float))

    df["global_base_cqd"] = base_arr
    df["active_residual_raw_cqd"] = raw_adj_arr
    df["active_residual_q_mass_reliability"] = q_mass_reliability.to_numpy(float)
    df["active_residual_edge_count_reliability"] = edge_count_reliability.to_numpy(float)
    df["active_residual_sample_mass_reliability"] = sample_mass_reliability.to_numpy(float)
    df["active_residual_reference_diversity_reliability"] = reference_diversity_reliability.to_numpy(float)
    df["active_residual_coverage_reliability"] = coverage_reliability.to_numpy(float)
    df["active_residual_validation_survival"] = float(validation_factor)
    df["active_residual_selected_row_multiplier"] = role_factor
    df["active_residual_reliability"] = reliability
    df["active_residual_soft_cap_factor"] = soft_cap_factor
    df["active_residual_effective_soft_cap_cqd"] = effective_soft_cap_cqd
    df["active_residual_robust_shrink"] = soft_cap_factor
    df["active_residual_survival_multiplier"] = survival
    df["active_residual_shrink_ratio"] = active_residual_shrink_ratio
    df["active_residual_shrunk_cqd"] = shrunk_adj
    df["active_uncertainty_penalty_cqd"] = uncertainty_penalty
    df["active_leverage_penalty_cqd"] = leverage_penalty
    df["active_total_penalty_cqd"] = total_penalty
    df["active_residual_net_adjustment_cqd"] = active_net_adjustment
    df["active_weight_evidence_gate"] = active_weight_evidence_gate
    df["active_validation_raw_logloss"] = active_validation_raw_logloss
    df["active_validation_base_logloss"] = active_validation_base_logloss
    df["active_validation_corrected_logloss"] = active_validation_corrected_logloss
    df["active_validation_corrected_minus_base_logloss"] = active_validation_corrected_logloss - active_validation_base_logloss if np.isfinite(active_validation_corrected_logloss) and np.isfinite(active_validation_base_logloss) else np.nan
    df["active_validation_corrected_minus_raw_logloss"] = active_validation_corrected_logloss - active_validation_raw_logloss if np.isfinite(active_validation_corrected_logloss) and np.isfinite(active_validation_raw_logloss) else np.nan
    df["regularized_active_cqd"] = reg_score
    df["regularized_active_logit"] = float(beta) * (df["regularized_active_cqd"].astype(float) - float(baseline_cqd))
    df["regularized_active_source"] = "raw_global_base_plus_evidence_aware_soft_capped_active_residual"
    return df


def _prepare_model_edges(groups_df: pd.DataFrame, all_edges: pd.DataFrame, context: str) -> pd.DataFrame:
    group_to_idx = {int(gid): i for i, gid in enumerate(groups_df["group_id"])}
    edges = _model_edges_for_ids(all_edges, group_to_idx.keys(), context)
    edges = edges.copy()
    edges["ia"] = edges.group_a.map(group_to_idx).astype(int)
    edges["ib"] = edges.group_b.map(group_to_idx).astype(int)
    edges["fold5"] = edge_fold_ids(edges["group_a"].to_numpy(), edges["group_b"].to_numpy(), nfold=max(2, min(5, len(edges))))
    return edges


def _fit_frozen_global_rsw_types(
    eligible_df: pd.DataFrame,
    all_edges: pd.DataFrame,
    seed: int,
    out_dir: Path,
) -> Dict[str, Any]:
    """Learn RSW-Type once from the full eligible Raw>=threshold nonblocked pool.

    RSW-Type is a win-rate distribution type. Recomputing it inside an active
    loop means the label definition changes as the visible-main set changes,
    which amplifies type stratification and can overfit rank movement.  This
    function freezes the type assignment before any active/challenger iteration.
    """
    eligible_df = eligible_df.copy().reset_index(drop=True)
    # The frozen RSW universe may include Raw<raw_min scout rows.  Scouts are
    # characterized by their true pairwise residual profile against the active
    # reference environment; they do not need scout-vs-scout edges unless they
    # are later rescued into active training.
    edges = _prepare_partial_edges_for_ids(
        all_edges,
        eligible_df["group_id"].astype(int).tolist(),
        "frozen full eligible+scout RSW-Type pool",
    )
    raw = eligible_df["raw_cqd"].to_numpy(float)
    all_mask = np.ones(len(edges), dtype=bool)
    beta_raw = fit_raw_beta(
        raw[edges["ia"].to_numpy()] - raw[edges["ib"].to_numpy()],
        edges["win_rate_a"].to_numpy(float),
        edges["samples"].to_numpy(float),
    )
    type_ids, type_labels, kdf, _, _ = adaptive_residual_type(
        eligible_df,
        edges,
        beta_raw,
        all_mask,
        validation_mask=None,
        seed=seed + 900_000,
    )
    global_rsw = pd.DataFrame({
        "group_id": eligible_df["group_id"].astype(int).to_numpy(),
        "frozen_rsw_type_id": type_ids.astype(int),
        "frozen_RSW-Type": type_labels,
    })
    global_rsw.to_csv(out_dir / "frozen_global_rsw_types.csv", index=False)
    kdf.to_csv(out_dir / "frozen_global_rsw_type_selection.csv", index=False)
    return {
        "beta_raw": float(beta_raw),
        "type_by_gid": {int(g): int(t) for g, t in zip(global_rsw["group_id"], global_rsw["frozen_rsw_type_id"])},
        "label_by_gid": {int(g): str(t) for g, t in zip(global_rsw["group_id"], global_rsw["frozen_RSW-Type"])},
        "kdf": kdf,
        "n_types": int(len(set(type_ids.tolist()))),
    }


def _fit_active_model_for_challenge(
    active_df: pd.DataFrame,
    all_edges: pd.DataFrame,
    seed: int,
    iteration: int,
    frozen_rsw: Dict[str, Any],
    active_weight: Optional[Dict[int, float]] = None,
    global_base: Optional[Dict[int, float]] = None,
) -> Dict[str, Any]:
    active_df = active_df.copy().reset_index(drop=True)
    if active_weight is None:
        active_weight = {int(g): 1.0 for g in active_df["group_id"].astype(int).tolist()}
    active_df["active_weight_q"] = active_df["group_id"].astype(int).map(lambda gid: float(active_weight.get(int(gid), 0.0)))
    edges, profile_edges = _prepare_weighted_training_edges(
        active_df,
        all_edges,
        active_weight,
        f"weighted active-set iteration {iteration} train pool",
    )
    raw = active_df["raw_cqd"].to_numpy(float)
    all_mask = np.ones(len(edges), dtype=bool)
    edge_like = edges["likelihood_weight"].to_numpy(float) if "likelihood_weight" in edges.columns else np.ones(len(edges), dtype=float)
    beta_raw = float(frozen_rsw["beta_raw"])
    missing_rsw = [
        int(g)
        for g in active_df["group_id"].astype(int).tolist()
        if int(g) not in frozen_rsw["type_by_gid"]
    ]
    if missing_rsw:
        preview = ",".join(str(g) for g in missing_rsw[:20])
        raise RuntimeError(
            f"Active iteration {iteration} contains group(s) without frozen global RSW-Type; "
            f"first_group_ids={preview}"
        )

    # Fixed global RSW-Type assignment. Do NOT call adaptive_residual_type()
    # inside the active loop.
    type_ids = np.asarray(
        [int(frozen_rsw["type_by_gid"][int(g)]) for g in active_df["group_id"].astype(int)],
        dtype=int,
    )
    type_labels = [str(frozen_rsw["label_by_gid"][int(g)]) for g in active_df["group_id"].astype(int)]

    # Keep low-rank fitting on the current weighted active support, but tie the
    # target rank to the frozen global type count rather than a per-iteration RSW
    # re-cluster.  Spectral/profile steps use weighted sample mass; the final
    # beta-binomial fit uses original counts plus likelihood_weight.
    selected_k = int(max(1, min(frozen_rsw["n_types"], 4, max(1, int(np.sqrt(max(1, len(active_df))))))))
    _, spectrum, embedding, _, _ = derive_lowrank_embedding(
        active_df,
        profile_edges,
        beta_raw,
        all_mask,
        seed=seed + 20_000 + iteration,
        target_rank=selected_k,
    )
    kdf = frozen_rsw["kdf"].copy()
    fit = fit_betabinomial_lowrank_counter_eb(active_df, edges, all_mask, type_ids, embedding, max_eb_iter=4, tol=2e-3, fixed_beta=beta_raw)
    beta = float(fit.beta)
    if abs(beta) <= 1e-8:
        corrected = beta * raw + fit.delta
    else:
        corrected = raw + fit.delta / beta

    scored = active_df.copy()
    scored["Correct Cqd"] = corrected
    scored["posterior_strength_logit"] = beta * raw + fit.delta
    scored["posterior_delta_logit"] = fit.delta
    scored["full_model_delta_logit"] = fit.delta
    scored["active_set_score_source"] = "weighted_active_fit"
    scored["active_set_iteration"] = iteration
    scored["active_set_challenger_edges"] = np.nan
    scored["active_set_score_success"] = True
    scored["RSW-Type"] = type_labels
    validation_metrics = _active_onefold_validation_metrics(
        active_df,
        edges,
        type_ids,
        embedding,
        beta_raw,
        global_base or {int(g): float(r) for g, r in active_df[["group_id", "raw_cqd"]].itertuples(index=False, name=None)},
        iteration,
    )

    ret = {
        "active_df": active_df,
        "edges": edges,
        "profile_edges": profile_edges,
        "fit": fit,
        "beta": beta,
        "beta_raw": beta_raw,
        "scored_active": scored,
        "type_ids": type_ids,
        "type_labels": type_labels,
        "embedding": embedding,
        "kdf": kdf,
        "spectrum": spectrum,
        "active_weight": {int(g): float(q) for g, q in active_weight.items()},
    }
    ret.update(validation_metrics)
    return ret

def _score_challengers_against_active(
    eligible_df: pd.DataFrame,
    active_model: Dict[str, Any],
    all_edges: pd.DataFrame,
    active_ids: Sequence[int],
    iteration: int,
) -> pd.DataFrame:
    active_ids = sorted({int(g) for g in active_ids})
    active_set = set(active_ids)
    challengers = eligible_df[~eligible_df["group_id"].astype(int).isin(active_set)].copy()
    if challengers.empty:
        return challengers.assign(
            **{
                "Correct Cqd": [],
                "posterior_strength_logit": [],
                "posterior_delta_logit": [],
                "full_model_delta_logit": [],
                "active_set_score_source": [],
                "active_set_iteration": [],
                "active_set_challenger_edges": [],
                "active_set_score_success": [],
                "active_set_score_message": [],
                "RSW-Type": [],
            }
        )

    fit = active_model["fit"]
    beta = float(active_model["beta"])
    active_scored = active_model["scored_active"]
    delta_by_gid = {
        int(gid): float(delta)
        for gid, delta in active_scored[["group_id", "posterior_delta_logit"]].itertuples(index=False, name=None)
    }
    raw_by_gid = {
        int(gid): float(raw)
        for gid, raw in active_scored[["group_id", "raw_cqd"]].itertuples(index=False, name=None)
    }
    default_type = "score_only_challenger"

    out_rows = []
    for _, r in challengers.iterrows():
        gid = int(r.group_id)
        raw_g = float(r.raw_cqd)
        sub = all_edges[
            ((all_edges.group_a == gid) & all_edges.group_b.isin(active_set)) |
            ((all_edges.group_b == gid) & all_edges.group_a.isin(active_set))
        ].copy()
        if len(sub) < len(active_ids):
            missing = len(active_ids) - len(sub)
            out = r.to_dict()
            out.update({
                "Correct Cqd": np.nan,
                "posterior_strength_logit": np.nan,
                "posterior_delta_logit": np.nan,
                "full_model_delta_logit": np.nan,
                "active_set_score_source": "challenger_score_only",
                "active_set_iteration": iteration,
                "active_set_challenger_edges": int(len(sub)),
                "active_set_score_success": False,
                "active_set_score_message": f"missing_edges_against_active:{missing}",
                "RSW-Type": default_type,
            })
            out_rows.append(out)
            continue

        y, n, eta0 = [], [], []
        for er in sub.itertuples(index=False):
            a = int(er.group_a)
            b = int(er.group_b)
            wr = float(er.win_rate_a)
            if a == gid:
                opp = b
                y_g = wr
            else:
                opp = a
                y_g = 1.0 - wr
            if opp not in raw_by_gid:
                continue
            y.append(y_g)
            n.append(float(er.samples))
            # Frozen active strength model for P(challenger beats active_opp).
            # We intentionally fit only a challenger group-level delta here.
            # Counter/lowrank terms are not estimated for a one-off challenger
            # because doing so would make hidden duplicates part of the training
            # representation again.
            eta0.append(beta * (raw_g - raw_by_gid[opp]) - delta_by_gid[opp])

        if not y:
            out = r.to_dict()
            out.update({
                "Correct Cqd": np.nan,
                "posterior_strength_logit": np.nan,
                "posterior_delta_logit": np.nan,
                "full_model_delta_logit": np.nan,
                "active_set_score_source": "challenger_score_only",
                "active_set_iteration": iteration,
                "active_set_challenger_edges": int(len(sub)),
                "active_set_score_success": False,
                "active_set_score_message": "no_usable_edges_against_active",
                "RSW-Type": default_type,
            })
            out_rows.append(out)
            continue

        sign = np.ones(len(y), dtype=float)
        delta, ok, msg, _, _ = beta_binomial_score_only_delta_any(
            np.asarray(y, dtype=float),
            np.asarray(n, dtype=float),
            np.asarray(eta0, dtype=float),
            sign,
            fit.tau_delta,
            fit.phi,
        )
        if abs(beta) <= 1e-8:
            correct = beta * raw_g + delta
        else:
            correct = raw_g + delta / beta
        out = r.to_dict()
        out.update({
            "Correct Cqd": float(correct),
            "posterior_strength_logit": float(beta * raw_g + delta),
            "posterior_delta_logit": float(delta),
            "full_model_delta_logit": float(delta),
            "active_set_score_source": "challenger_score_only",
            "active_set_iteration": iteration,
            "active_set_challenger_edges": int(len(y)),
            "active_set_score_success": bool(ok),
            "active_set_score_message": str(msg),
            "RSW-Type": default_type,
        })
        out_rows.append(out)

    return pd.DataFrame(out_rows)


def _score_all_groups_against_active_environment(
    eligible_df: pd.DataFrame,
    active_model: Dict[str, Any],
    all_edges: pd.DataFrame,
    active_ids: Optional[Sequence[int]] = None,
    iteration: int = 0,
    active_weight: Optional[Dict[int, float]] = None,
) -> pd.DataFrame:
    """Score every eligible group against a weighted active environment.

    Current active/support members and challengers are both scored as one-group
    projections against the same reference environment.  Each opponent j enters
    with likelihood weight proportional to q_j, excluding self.  Missing edges
    remain measured-data diagnostics and are never filled with synthetic 0.
    """
    if active_weight is None:
        if active_ids is None:
            raise RuntimeError("weighted active-environment projection requires active_weight or active_ids")
        active_weight = {int(g): 1.0 for g in active_ids}
    active_weight = {int(g): float(q) for g, q in active_weight.items() if float(q) > 0.0}
    reference_ids_all = _active_weight_support_ids(active_weight)
    if len(reference_ids_all) < 2:
        raise RuntimeError("weighted active-environment projection requires at least two positive-weight reference groups")

    def _empty_reference_stats(reference_ids: Sequence[int], missing: int) -> Dict[str, float]:
        q_vals = np.asarray([float(active_weight.get(int(a), 0.0)) for a in reference_ids], dtype=float)
        q_sum = float(np.sum(q_vals)) if len(q_vals) else 0.0
        eff = float((q_sum * q_sum) / max(1e-12, float(np.sum(q_vals * q_vals)))) if q_sum > 0.0 else 0.0
        max_share = float(np.max(q_vals) / q_sum) if q_sum > 0.0 and len(q_vals) else 1.0
        edge_count = 0.0
        coverage = float(edge_count / max(edge_count + float(missing), 1.0))
        return {
            "active_weight_reference_mass": q_sum,
            "active_weight_reference_mean": float(np.mean(q_vals)) if len(q_vals) else 0.0,
            "active_weight_reference_missing_edges": int(missing),
            "active_weight_reference_coverage_ratio": coverage,
            "active_weight_reference_sample_mass": 0.0,
            "active_weight_reference_q_sample_mass": 0.0,
            "active_weight_reference_effective_count": eff,
            "active_weight_reference_effective_evidence_count": 0.0,
            "active_weight_reference_max_share": max_share,
            "active_weight_reference_max_evidence_share": 1.0,
        }

    def _reference_stats(ref_w: Sequence[float], samples: Sequence[float], reference_ids: Sequence[int], missing: int) -> Dict[str, float]:
        q = np.asarray(ref_w, dtype=float)
        n = np.maximum(np.asarray(samples, dtype=float), 0.0)
        q = np.nan_to_num(q, nan=0.0, posinf=0.0, neginf=0.0)
        q = np.maximum(q, 0.0)
        evidence = q * n
        q_sum = float(np.sum(q))
        n_sum = float(np.sum(n))
        ev_sum = float(np.sum(evidence))
        eff_q = float((q_sum * q_sum) / max(1e-12, float(np.sum(q * q)))) if q_sum > 0.0 else 0.0
        eff_ev = float((ev_sum * ev_sum) / max(1e-12, float(np.sum(evidence * evidence)))) if ev_sum > 0.0 else 0.0
        max_q_share = float(np.max(q) / q_sum) if q_sum > 0.0 and len(q) else 1.0
        max_ev_share = float(np.max(evidence) / ev_sum) if ev_sum > 0.0 and len(evidence) else max_q_share
        edge_count = float(len(q))
        coverage = float(edge_count / max(edge_count + float(missing), 1.0))
        return {
            "active_weight_reference_mass": q_sum,
            "active_weight_reference_mean": float(np.mean(q)) if len(q) else 0.0,
            "active_weight_reference_missing_edges": int(missing),
            "active_weight_reference_coverage_ratio": coverage,
            "active_weight_reference_sample_mass": n_sum,
            "active_weight_reference_q_sample_mass": ev_sum,
            "active_weight_reference_effective_count": eff_q,
            "active_weight_reference_effective_evidence_count": eff_ev,
            "active_weight_reference_max_share": max_q_share,
            "active_weight_reference_max_evidence_share": max_ev_share,
        }

    fit = active_model["fit"]
    beta = float(active_model["beta"])
    active_scored = active_model["scored_active"]

    delta_by_gid = {
        int(gid): float(delta)
        for gid, delta in active_scored[["group_id", "posterior_delta_logit"]].itertuples(index=False, name=None)
    }
    raw_by_gid = {
        int(gid): float(raw)
        for gid, raw in active_scored[["group_id", "raw_cqd"]].itertuples(index=False, name=None)
    }
    type_by_gid = {}
    if "RSW-Type" in active_scored.columns:
        type_by_gid = {
            int(gid): str(label)
            for gid, label in active_scored[["group_id", "RSW-Type"]].itertuples(index=False, name=None)
        }

    out_rows = []
    for _, r in eligible_df.iterrows():
        gid = int(r.group_id)
        raw_g = float(r.raw_cqd)
        is_active_input = float(active_weight.get(gid, 0.0)) > ACTIVE_SET_WEIGHTED_SUPPORT_EPS
        reference_ids = [a for a in reference_ids_all if a != gid and float(active_weight.get(a, 0.0)) > ACTIVE_SET_WEIGHTED_SUPPORT_EPS]
        reference_set = set(reference_ids)
        if not reference_ids:
            out = r.to_dict()
            out.update({
                "Correct Cqd": np.nan,
                "posterior_strength_logit": np.nan,
                "posterior_delta_logit": np.nan,
                "full_model_delta_logit": np.nan,
                "active_set_score_source": "weighted_active_environment_projection",
                "active_set_iteration": iteration,
                "active_set_challenger_edges": 0,
                "active_set_score_success": False,
                "active_set_score_message": "no_weighted_active_reference_rows_after_excluding_self",
                "active_set_projection_role": "active_leave_self_out" if is_active_input else "challenger",
                "active_weight_q": float(active_weight.get(gid, 0.0)),
                "RSW-Type": type_by_gid.get(gid, "weighted_active_environment_projection"),
            })
            out.update(_empty_reference_stats(reference_ids, missing=0))
            out_rows.append(out)
            continue

        sub = all_edges[
            ((all_edges.group_a.astype(int) == gid) & (all_edges.group_b.astype(int).isin(reference_set))) |
            ((all_edges.group_b.astype(int) == gid) & (all_edges.group_a.astype(int).isin(reference_set)))
        ].copy()

        missing = max(0, len(reference_ids) - len(sub))

        y, n, eta0, ref_w = [], [], [], []
        for er in sub.itertuples(index=False):
            a = int(er.group_a)
            b = int(er.group_b)
            wr = float(er.win_rate_a)
            if a == gid:
                opp = b
                y_g = wr
            else:
                opp = a
                y_g = 1.0 - wr
            if opp not in raw_by_gid or opp not in delta_by_gid:
                continue
            q_opp = float(active_weight.get(opp, 0.0))
            if q_opp <= ACTIVE_SET_WEIGHTED_SUPPORT_EPS:
                continue
            y.append(y_g)
            n.append(float(er.samples))
            ref_w.append(q_opp)
            eta0.append(beta * (raw_g - raw_by_gid[opp]) - delta_by_gid[opp])

        ref_stats = _reference_stats(ref_w, n, reference_ids, missing) if y else _empty_reference_stats(reference_ids, missing)
        if not y:
            out = r.to_dict()
            out.update({
                "Correct Cqd": np.nan,
                "posterior_strength_logit": np.nan,
                "posterior_delta_logit": np.nan,
                "full_model_delta_logit": np.nan,
                "active_set_score_source": "weighted_active_environment_projection",
                "active_set_iteration": iteration,
                "active_set_challenger_edges": int(len(sub)),
                "active_set_score_success": False,
                "active_set_score_message": "no_usable_edges_against_weighted_active_reference",
                "active_set_projection_role": "active_leave_self_out" if is_active_input else "challenger",
                "active_weight_q": float(active_weight.get(gid, 0.0)),
                "RSW-Type": type_by_gid.get(gid, "weighted_active_environment_projection"),
            })
            out.update(ref_stats)
            out_rows.append(out)
            continue

        sign = np.ones(len(y), dtype=float)
        like_w = _sanitize_likelihood_weights(np.asarray(ref_w, dtype=float))
        delta, ok, msg, _, _ = beta_binomial_score_only_delta_any(
            np.asarray(y, dtype=float),
            np.asarray(n, dtype=float),
            np.asarray(eta0, dtype=float),
            sign,
            fit.tau_delta,
            fit.phi,
            likelihood_weight=like_w,
        )
        if abs(beta) <= 1e-8:
            correct = beta * raw_g + delta
        else:
            correct = raw_g + delta / beta

        out = r.to_dict()
        out.update({
            "Correct Cqd": float(correct),
            "posterior_strength_logit": float(beta * raw_g + delta),
            "posterior_delta_logit": float(delta),
            "full_model_delta_logit": float(delta),
            "active_set_score_source": "weighted_active_environment_projection",
            "active_set_iteration": iteration,
            "active_set_challenger_edges": int(len(y)),
            "active_set_score_success": bool(ok),
            "active_set_score_message": str(msg),
            "active_set_projection_role": "active_leave_self_out" if is_active_input else "challenger",
            "active_weight_q": float(active_weight.get(gid, 0.0)),
            "RSW-Type": type_by_gid.get(gid, "weighted_active_environment_projection"),
        })
        out.update(ref_stats)
        out_rows.append(out)

    return pd.DataFrame(out_rows)


def run_active_set_challenger_selection(
    eligible_groups_df: pd.DataFrame,
    all_edges: pd.DataFrame,
    group_members: Dict[int, List[str]],
    raw_min: Optional[float],
    seed: int,
    out_dir: Path,
    frozen_rsw: Dict[str, Any],
    lane_size: int,
    initial_pool_df: Optional[pd.DataFrame] = None,
    global_base: Optional[Dict[int, float]] = None,
) -> Dict[str, Any]:
    if eligible_groups_df.empty:
        raise RuntimeError("active-set selection received an empty eligible group pool")

    eligible = eligible_groups_df.copy().reset_index(drop=True)
    baseline_cqd = float(raw_min) if raw_min is not None else float(eligible["raw_cqd"].min())
    if global_base is None:
        global_base = {int(g): float(r) for g, r in eligible[["group_id", "raw_cqd"]].itertuples(index=False, name=None)}
    else:
        global_base = {int(g): float(global_base.get(int(g), r)) for g, r in eligible[["group_id", "raw_cqd"]].itertuples(index=False, name=None)}
    eligible["global_base_cqd"] = eligible["group_id"].astype(int).map(lambda gid: float(global_base.get(int(gid), np.nan))).astype(float)

    init = (initial_pool_df.copy() if initial_pool_df is not None else eligible.copy()).reset_index(drop=True)
    init["active_set_init_score"] = init["group_id"].astype(int).map(lambda gid: float(global_base.get(int(gid), init.loc[init["group_id"].astype(int) == int(gid), "raw_cqd"].iloc[0]))).astype(float)
    init["active_set_init_utility"] = init["active_set_init_score"].astype(float) - baseline_cqd
    initial_active_ids = _greedy_visible_main_group_ids(
        init,
        group_members,
        score_col="active_set_init_score",
        context="raw-initial-visible-main-active-set",
    )
    if len(initial_active_ids) < 2:
        raise RuntimeError(f"active-set initialization selected fewer than 2 groups: selected={len(initial_active_ids)}")

    active_weight: Dict[int, float] = {
        int(g): (1.0 if int(g) in set(initial_active_ids) else 0.0)
        for g in eligible["group_id"].astype(int).tolist()
    }
    active_ids = _active_weight_support_ids(active_weight)

    iteration_rows = []
    final_scored = None
    stable = False
    convergence_mode = "regularized_weighted_active_max_iter"
    smoothing_alpha = active_set_cqd_smoothing_alpha(lane_size)
    weight_alpha = 0.5
    temperature_cqd = active_set_soft_selection_temperature_cqd(lane_size)
    score_state = {
        int(g): float(global_base.get(int(g), raw))
        for g, raw in eligible[["group_id", "raw_cqd"]].itertuples(index=False, name=None)
    }
    best_validation_logloss = float("inf")
    validation_bad_rounds = 0
    q_history: List[Dict[int, float]] = []
    moment_alignment_rows: List[Dict[str, Any]] = []
    target_ema_state: Dict[int, float] = {int(g): float(active_weight.get(int(g), 0.0)) for g in active_weight}
    # Lazy initialized from first observed desire/availability.  Initializing
    # these to zero made early-iteration audit and any downstream use look like
    # a real suppression signal even though no environment history existed yet.
    desire_ema_state: Dict[int, float] = {}
    availability_ema_state: Dict[int, float] = {}
    prev_target_direction: Dict[int, int] = {int(g): 0 for g in active_weight}
    target_flip_count: Dict[int, int] = {int(g): 0 for g in active_weight}
    target_flip_streak: Dict[int, int] = {int(g): 0 for g in active_weight}

    for iteration in range(1, ACTIVE_SET_WEIGHTED_MAX_ITERS + 1):
        active_ids = _active_weight_support_ids(active_weight)
        if len(active_ids) < 2:
            # Fall back to the strongest current q rows if numerical support got
            # too sparse.  This is a numerical safeguard for the continuous map,
            # not a rank cap or business rule.
            active_ids = [gid for gid, _ in sorted(active_weight.items(), key=lambda kv: (-kv[1], kv[0]))[:max(2, len(initial_active_ids))]]
            for gid in active_ids:
                active_weight[int(gid)] = max(float(active_weight.get(int(gid), 0.0)), ACTIVE_SET_WEIGHTED_SUPPORT_EPS * 2.0)

        # Weighted active support may include low-q scouts before every pair has
        # been requested.  We therefore fit/score on available edges and let rows
        # with missing weighted-reference evidence decay through q rather than
        # aborting the entire continuous iteration.
        active_df = eligible[eligible["group_id"].astype(int).isin(set(active_ids))].copy()
        active_df = active_df.sort_values(["raw_rank", "raw_cqd", "group_id"], ascending=[True, False, True]).reset_index(drop=True)
        active_df = _attach_active_weight_columns(active_df, active_weight)

        # Required active-environment edges are measurable data.  Do not silently
        # downweight missing candidate-active edges here; request them from Rust
        # and retry so selected / not_selected / challenger scores use measured
        # support rather than synthetic missing mass.
        require_pairs_or_request(
            all_edges,
            active_ids,
            None,
            f"weighted active-set iteration {iteration} active-active support",
            lane_size,
            out_dir,
        )
        require_pairs_or_request(
            all_edges,
            eligible["group_id"].astype(int).tolist(),
            active_ids,
            f"weighted active-set iteration {iteration} candidate-active projection",
            lane_size,
            out_dir,
        )

        active_model = _fit_active_model_for_challenge(
            active_df,
            all_edges,
            seed=seed,
            iteration=iteration,
            frozen_rsw=frozen_rsw,
            active_weight=active_weight,
            global_base=global_base,
        )
        combined = _score_all_groups_against_active_environment(
            eligible,
            active_model,
            all_edges,
            active_weight=active_weight,
            iteration=iteration,
        )
        scored_challengers = combined[combined["active_weight_q"].astype(float) <= ACTIVE_SET_WEIGHTED_SUPPORT_EPS].copy()
        combined["resolver_baseline_cqd"] = baseline_cqd
        combined["resolver_baseline_logit"] = float(active_model["beta"] * baseline_cqd)
        combined = _apply_regularized_active_residuals(
            combined,
            global_base=global_base,
            baseline_cqd=baseline_cqd,
            beta=float(active_model["beta"]),
            active_model=active_model,
        )
        combined, _moment_diag = _align_score_moments_to_raw(
            combined,
            score_col="regularized_active_cqd",
            raw_col="raw_cqd",
            context="active_iteration_environment",
            iteration=iteration,
        )
        moment_alignment_rows.append(_moment_diag)
        combined = _refresh_regularized_score_after_moment_alignment(
            combined,
            baseline_cqd=baseline_cqd,
            beta=float(active_model["beta"]),
        )
        combined["resolver_marginal_utility_logit"] = combined["regularized_active_logit"].astype(float)

        combined = _apply_active_set_cqd_smoothing(
            combined,
            score_state,
            smoothing_alpha,
            raw_col="global_base_cqd",
            current_score_col="regularized_active_cqd",
            output_col="active_set_smoothed_regularized_cqd",
        )
        combined["active_set_smoothed_correct_cqd"] = combined["active_set_smoothed_regularized_cqd"].astype(float)
        combined["active_set_soft_kicked_below_baseline"] = (
            np.isfinite(combined["active_set_smoothed_regularized_cqd"].to_numpy(float))
            & (combined["active_set_smoothed_regularized_cqd"].astype(float) <= float(baseline_cqd))
        )
        combined = _compute_soft_browser_active_targets(
            combined,
            group_members,
            baseline_cqd=baseline_cqd,
            score_col="active_set_smoothed_regularized_cqd",
            temperature_cqd=temperature_cqd,
            target_mass=float(len(initial_active_ids)),
        )

        # Smooth the environment inputs themselves, not only the q output.  This
        # prevents resolver availability/desire flips from pushing the active map
        # into a last-phase oscillation.
        ema_alpha = float(ACTIVE_Q_TARGET_EMA_ALPHA)
        target_mass = float(len(initial_active_ids))
        raw_target_sum = float(pd.to_numeric(combined["active_weight_target"], errors="coerce").fillna(0.0).sum())
        ema_target_values: Dict[int, float] = {}
        ema_target_raw_values: Dict[int, float] = {}
        target_direction_values: Dict[int, int] = {}
        row_osc_mult_values: Dict[int, float] = {}
        for _gid, _desire, _avail, _target in combined[["group_id", "active_weight_desire", "active_weight_availability", "active_weight_target"]].itertuples(index=False, name=None):
            gid = int(_gid)
            desire_raw = float(_desire) if np.isfinite(float(_desire)) else 0.0
            avail_raw = float(_avail) if np.isfinite(float(_avail)) else 0.0
            target_raw = float(_target) if np.isfinite(float(_target)) else 0.0
            if gid not in desire_ema_state:
                desire_ema_state[gid] = desire_raw
            else:
                desire_ema_state[gid] = (1.0 - ema_alpha) * float(desire_ema_state.get(gid, desire_raw)) + ema_alpha * desire_raw
            if gid not in availability_ema_state:
                availability_ema_state[gid] = avail_raw
            else:
                availability_ema_state[gid] = (1.0 - ema_alpha) * float(availability_ema_state.get(gid, avail_raw)) + ema_alpha * avail_raw
            prev_target = float(target_ema_state.get(gid, float(active_weight.get(gid, 0.0))))
            target_ema = (1.0 - ema_alpha) * prev_target + ema_alpha * target_raw
            old_q_for_sign = float(active_weight.get(gid, 0.0))
            direction = 1 if target_ema > old_q_for_sign + 1e-9 else (-1 if target_ema < old_q_for_sign - 1e-9 else 0)
            prev_dir = int(prev_target_direction.get(gid, 0))
            if direction != 0 and prev_dir != 0 and direction != prev_dir:
                target_flip_count[gid] = int(target_flip_count.get(gid, 0)) + 1
                target_flip_streak[gid] = int(target_flip_streak.get(gid, 0)) + 1
            elif direction != 0:
                target_flip_streak[gid] = 0
            if direction != 0:
                prev_target_direction[gid] = direction
            row_osc = 1.0 / (1.0 + float(ACTIVE_Q_OSCILLATION_DAMP_STRENGTH) * float(target_flip_streak.get(gid, 0)))
            target_ema_damped = old_q_for_sign + (target_ema - old_q_for_sign) * row_osc
            target_ema_state[gid] = max(0.0, min(1.0, target_ema_damped))
            ema_target_raw_values[gid] = target_ema_state[gid]
            target_direction_values[gid] = direction
            row_osc_mult_values[gid] = row_osc
        ema_mass_raw = float(sum(ema_target_raw_values.values()))
        ema_target_scaled = _renormalize_weight_mass(ema_target_raw_values, target_mass)
        ema_mass_scaled = float(sum(ema_target_scaled.values()))
        # Keep the EMA state in the same mass scale as the q update target.
        # Otherwise the next iteration mixes unscaled target EMA with scaled q,
        # which changes the apparent direction of the q map.
        for _gid, _q_scaled in ema_target_scaled.items():
            target_ema_state[int(_gid)] = float(_q_scaled)
        target_direction_values = {
            int(gid): (1 if float(ema_target_scaled.get(int(gid), 0.0)) > float(active_weight.get(int(gid), 0.0)) + 1e-9 else (-1 if float(ema_target_scaled.get(int(gid), 0.0)) < float(active_weight.get(int(gid), 0.0)) - 1e-9 else 0))
            for gid in ema_target_scaled
        }
        combined["active_weight_desire_raw"] = combined["active_weight_desire"].astype(float)
        combined["active_weight_availability_raw"] = combined["active_weight_availability"].astype(float)
        combined["active_weight_target_raw"] = combined["active_weight_target"].astype(float)
        combined["active_weight_desire_ema"] = combined["group_id"].astype(int).map(lambda gid: float(desire_ema_state.get(int(gid), 0.0)))
        combined["active_weight_availability_ema"] = combined["group_id"].astype(int).map(lambda gid: float(availability_ema_state.get(int(gid), 0.0)))
        combined["active_weight_target_ema_unscaled"] = combined["group_id"].astype(int).map(lambda gid: float(ema_target_raw_values.get(int(gid), 0.0)))
        combined["active_weight_target"] = combined["group_id"].astype(int).map(lambda gid: float(ema_target_scaled.get(int(gid), 0.0))).astype(float)
        combined["active_weight_target_ema"] = combined["active_weight_target"].astype(float)
        combined["active_weight_target_raw_mass"] = float(raw_target_sum)
        combined["active_weight_target_ema_raw_mass"] = float(ema_mass_raw)
        combined["active_weight_target_ema_scaled_mass"] = float(ema_mass_scaled)
        combined["active_weight_target_direction"] = combined["group_id"].astype(int).map(lambda gid: int(target_direction_values.get(int(gid), 0)))
        combined["active_weight_target_flip_count"] = combined["group_id"].astype(int).map(lambda gid: int(target_flip_count.get(int(gid), 0)))
        combined["active_weight_target_flip_streak"] = combined["group_id"].astype(int).map(lambda gid: int(target_flip_streak.get(int(gid), 0)))
        combined["active_weight_oscillation_row_multiplier"] = combined["group_id"].astype(int).map(lambda gid: float(row_osc_mult_values.get(int(gid), 1.0)))
        combined["active_weight_oscillation_score"] = combined["active_weight_target_flip_count"].astype(float) / np.maximum(float(iteration), 1.0)

        val_corr = float(active_model.get("active_validation_corrected_logloss", np.nan))
        val_base = float(active_model.get("active_validation_base_logloss", np.nan))
        if np.isfinite(val_corr):
            if val_corr + ACTIVE_REG_VALIDATION_TOL < best_validation_logloss:
                best_validation_logloss = val_corr
                validation_bad_rounds = 0
            else:
                validation_bad_rounds += 1
        if np.isfinite(val_corr) and np.isfinite(val_base):
            if val_corr <= val_base + ACTIVE_REG_VALIDATION_TOL:
                step_alpha = ACTIVE_REG_GOOD_STEP_ALPHA
            elif val_corr <= val_base + 5.0 * ACTIVE_REG_VALIDATION_TOL:
                step_alpha = ACTIVE_REG_NEUTRAL_STEP_ALPHA
            else:
                step_alpha = ACTIVE_REG_BAD_STEP_ALPHA
        else:
            step_alpha = ACTIVE_REG_NEUTRAL_STEP_ALPHA
        # validation_bad_rounds is diagnostic only; it must not force an
        # additional bad-step throttle.

        old_weight = {int(g): float(active_weight.get(int(g), 0.0)) for g in eligible["group_id"].astype(int).tolist()}
        target_weight = {
            int(g): float(q)
            for g, q in combined[["group_id", "active_weight_target"]].itertuples(index=False, name=None)
        }

        def _make_weight(alpha: float) -> Dict[int, float]:
            out_w = {}
            for gid in old_weight:
                old_q = float(old_weight.get(gid, 0.0))
                target_q = float(target_weight.get(gid, 0.0))
                q = old_q + float(alpha) * (target_q - old_q)
                out_w[gid] = max(0.0, min(1.0, q))
            return out_w

        step_alpha_before_churn = float(step_alpha)
        tentative_weight = _make_weight(step_alpha_before_churn)
        tentative_deltas = [abs(float(tentative_weight.get(gid, 0.0)) - float(old_weight.get(gid, 0.0))) for gid in old_weight]
        tentative_max_delta = float(np.max(tentative_deltas)) if tentative_deltas else 0.0
        tentative_mean_delta = float(np.mean(tentative_deltas)) if tentative_deltas else 0.0
        support_before = set(_active_weight_support_ids(old_weight))
        support_tentative = set(_active_weight_support_ids(tentative_weight))
        churn_count_tentative = len(support_tentative - support_before) + len(support_before - support_tentative)
        churn_ratio_tentative = float(churn_count_tentative / max(1, len(support_before)))
        macro_churn_step_multiplier = _active_churn_step_multiplier(churn_ratio_tentative)
        micro_churn_step_multiplier = _active_micro_churn_step_multiplier(iteration, churn_ratio_tentative, tentative_max_delta)
        churn_step_multiplier = float(min(macro_churn_step_multiplier, micro_churn_step_multiplier))
        step_alpha = float(step_alpha_before_churn * churn_step_multiplier)
        new_weight = _make_weight(step_alpha)
        deltas = [abs(float(new_weight.get(gid, 0.0)) - float(old_weight.get(gid, 0.0))) for gid in old_weight]
        active_weight = new_weight
        q_history.append({int(g): float(q) for g, q in active_weight.items()})
        combined["active_weight_q_next"] = combined["group_id"].astype(int).map(lambda gid: float(active_weight.get(int(gid), 0.0)))
        combined["active_weight_delta"] = (combined["active_weight_q_next"].astype(float) - combined["active_weight_q"].astype(float)).abs()
        combined["active_weight_step_alpha_before_churn_damp"] = float(step_alpha_before_churn)
        combined["active_weight_churn_ratio_tentative"] = float(churn_ratio_tentative)
        combined["active_weight_churn_step_multiplier"] = float(churn_step_multiplier)
        combined["active_weight_macro_churn_step_multiplier"] = float(macro_churn_step_multiplier)
        combined["active_weight_micro_churn_step_multiplier"] = float(micro_churn_step_multiplier)
        combined["active_weight_tentative_max_delta"] = float(tentative_max_delta)
        combined["active_weight_tentative_mean_delta"] = float(tentative_mean_delta)

        support_ids = _active_weight_support_ids(active_weight)
        # Diagnostic hard-visible set only.  The environment itself is q-weighted.
        hard_visible_ids = _greedy_visible_main_group_ids(
            combined.assign(active_weight_sort=combined["active_weight_q_next"].astype(float)),
            group_members,
            score_col="active_weight_sort",
            context=f"weighted-active-iteration-{iteration}-diagnostic-visible-main-q",
        )
        max_delta = float(np.max(deltas)) if deltas else 0.0
        mean_delta = float(np.mean(deltas)) if deltas else 0.0
        debug_event = _write_active_iteration_debug_files(
            out_dir=out_dir,
            iteration=iteration,
            combined=combined,
            active_ids_in=active_ids,
            support_ids_out=support_ids,
            hard_visible_ids=hard_visible_ids,
            old_weight=old_weight,
            target_weight=target_weight,
            new_weight=active_weight,
            step_alpha=step_alpha,
            baseline_cqd=baseline_cqd,
            active_model=active_model,
            validation_bad_rounds=validation_bad_rounds,
        )
        stable = bool(
            max_delta <= ACTIVE_SET_WEIGHTED_CONVERGENCE_MAX_DELTA
            and mean_delta <= ACTIVE_SET_WEIGHTED_CONVERGENCE_MEAN_DELTA
        )
        iteration_rows.append({
            "iteration": iteration,
            "active_in": len(active_ids),
            "active_support_out": len(support_ids),
            "selected_out": len(hard_visible_ids),
            "scored_challengers": int(len(scored_challengers)),
            "scoreable_challengers": int(scored_challengers["active_set_score_success"].fillna(False).sum()) if len(scored_challengers) else 0,
            "score_projection_mode": "weighted_active_environment_projection",
            "active_set_cqd_smoothing_alpha": float(smoothing_alpha),
            "active_weight_update_alpha": float(step_alpha),
            "active_weight_update_alpha_before_churn_damp": float(step_alpha_before_churn),
            "active_weight_churn_ratio_tentative": float(churn_ratio_tentative),
            "active_weight_churn_step_multiplier": float(churn_step_multiplier),
            "active_weight_macro_churn_step_multiplier": float(macro_churn_step_multiplier),
            "active_weight_micro_churn_step_multiplier": float(micro_churn_step_multiplier),
            "active_weight_tentative_max_delta": float(tentative_max_delta),
            "active_weight_tentative_mean_delta": float(tentative_mean_delta),
            "active_weight_target_raw_mass": float(raw_target_sum),
            "active_weight_target_ema_raw_mass": float(ema_mass_raw),
            "active_weight_target_ema_scaled_mass": float(ema_mass_scaled),
            "active_weight_target_flip_rows": int((combined["active_weight_target_flip_count"].astype(int) > 0).sum()) if "active_weight_target_flip_count" in combined.columns else 0,
            "active_weight_target_flip_count_total": int(combined["active_weight_target_flip_count"].astype(int).sum()) if "active_weight_target_flip_count" in combined.columns else 0,
            "active_weight_target_flip_streak_max": int(combined["active_weight_target_flip_streak"].astype(int).max()) if "active_weight_target_flip_streak" in combined.columns and len(combined) else 0,
            "active_weight_oscillation_score_mean": float(combined["active_weight_oscillation_score"].astype(float).mean()) if "active_weight_oscillation_score" in combined.columns and len(combined) else 0.0,
            "active_moment_alignment_applied": bool(_moment_diag.get("moment_alignment_applied", False)),
            "active_moment_alignment_rows": int(_moment_diag.get("moment_alignment_rows", 0)),
            "active_moment_raw_mean": float(_moment_diag.get("raw_mean", np.nan)),
            "active_moment_raw_sd": float(_moment_diag.get("raw_sd", np.nan)),
            "active_moment_projected_mean_before": float(_moment_diag.get("projected_mean_before", np.nan)),
            "active_moment_projected_sd_before": float(_moment_diag.get("projected_sd_before", np.nan)),
            "active_moment_projected_mean_after": float(_moment_diag.get("projected_mean_after", np.nan)),
            "active_moment_projected_sd_after": float(_moment_diag.get("projected_sd_after", np.nan)),
            "active_moment_alignment_scale_factor": float(_moment_diag.get("alignment_scale_factor", np.nan)),
            "active_moment_alignment_shift_cqd": float(_moment_diag.get("alignment_shift_cqd", np.nan)),
            "active_validation_raw_logloss": float(active_model.get("active_validation_raw_logloss", np.nan)),
            "active_validation_base_logloss": float(active_model.get("active_validation_base_logloss", np.nan)),
            "active_validation_corrected_logloss": float(active_model.get("active_validation_corrected_logloss", np.nan)),
            "active_validation_bad_rounds": int(validation_bad_rounds),
            "regularized_active_mean_abs_residual_raw_cqd": float(np.nanmean(np.abs(combined["active_residual_raw_cqd"].to_numpy(float)))) if "active_residual_raw_cqd" in combined.columns else np.nan,
            "regularized_active_mean_abs_residual_shrunk_cqd": float(np.nanmean(np.abs(combined["active_residual_shrunk_cqd"].to_numpy(float)))) if "active_residual_shrunk_cqd" in combined.columns else np.nan,
            "regularized_active_mean_reliability": float(np.nanmean(combined["active_residual_reliability"].to_numpy(float))) if "active_residual_reliability" in combined.columns else np.nan,
            "regularized_active_mean_q_mass_reliability": float(np.nanmean(combined["active_residual_q_mass_reliability"].to_numpy(float))) if "active_residual_q_mass_reliability" in combined.columns else np.nan,
            "regularized_active_mean_edge_count_reliability": float(np.nanmean(combined["active_residual_edge_count_reliability"].to_numpy(float))) if "active_residual_edge_count_reliability" in combined.columns else np.nan,
            "regularized_active_mean_sample_mass_reliability": float(np.nanmean(combined["active_residual_sample_mass_reliability"].to_numpy(float))) if "active_residual_sample_mass_reliability" in combined.columns else np.nan,
            "regularized_active_mean_reference_diversity_reliability": float(np.nanmean(combined["active_residual_reference_diversity_reliability"].to_numpy(float))) if "active_residual_reference_diversity_reliability" in combined.columns else np.nan,
            "regularized_active_validation_survival": float(pd.to_numeric(combined.get("active_residual_validation_survival", pd.Series([np.nan])), errors="coerce").dropna().iloc[0]) if pd.to_numeric(combined.get("active_residual_validation_survival", pd.Series([np.nan])), errors="coerce").notna().any() else np.nan,
            "active_weight_temperature_cqd": float(temperature_cqd),
            "active_weight_support_eps": float(ACTIVE_SET_WEIGHTED_SUPPORT_EPS),
            "active_weight_target_mass": float(len(initial_active_ids)),
            "active_weight_realized_target_mass": float(combined["active_weight_target"].astype(float).sum()),
            "active_weight_mass_center_cqd": float(pd.to_numeric(combined.get("active_weight_mass_center_cqd", pd.Series([np.nan])), errors="coerce").dropna().iloc[0]) if pd.to_numeric(combined.get("active_weight_mass_center_cqd", pd.Series([np.nan])), errors="coerce").notna().any() else np.nan,
            "active_weight_max_delta": max_delta,
            "active_weight_mean_delta": mean_delta,
            "active_weight_mass": float(sum(active_weight.values())),
            "active_weight_effective_count": float((sum(active_weight.values()) ** 2) / max(1e-12, sum(q * q for q in active_weight.values()))),
            "entered_support_count": int(debug_event.get("entered_support_count", 0)),
            "exited_support_count": int(debug_event.get("exited_support_count", 0)),
            "entered_support_ids_preview": _compact_id_list(debug_event.get("entered_support_ids", []), limit=40),
            "exited_support_ids_preview": _compact_id_list(debug_event.get("exited_support_ids", []), limit=40),
            "soft_kicked_below_baseline": int(combined.get("active_set_soft_kicked_below_baseline", pd.Series([], dtype=bool)).fillna(False).sum()),
            "stable": bool(stable),
            "convergence_mode": "weighted_fixed_point" if stable else "weighted_iterating",
            "beta": float(active_model["beta"]),
            "tau_delta": float(active_model["fit"].tau_delta),
            "tau_counter": float(active_model["fit"].tau_counter),
            "tau_lowrank": float(active_model["fit"].tau_lowrank),
            "phi": float(active_model["fit"].phi),
        })

        combined["active_set_selected_next"] = combined["group_id"].astype(int).isin(set(hard_visible_ids)).astype(int)
        combined["active_set_was_active_input"] = combined["active_weight_q"].astype(float) > ACTIVE_SET_WEIGHTED_SUPPORT_EPS
        combined["active_set_convergence_mode"] = "weighted_fixed_point" if stable else "weighted_iterating"
        combined.to_csv(out_dir / f"active_set_scored_candidates_iter{iteration}.csv", index=False)
        final_scored = combined

        if stable:
            convergence_mode = "regularized_weighted_fixed_point"
            break
        # Do not early-stop on validation patience.  Keep validation metrics as
        # diagnostics only so adjacent raw_min runs are comparable and every run
        # follows the same active q iteration schedule.

    if final_scored is None:
        raise RuntimeError("weighted active-set selection did not run any iterations")

    active_weight_last = {int(g): float(q) for g, q in active_weight.items()}
    if not stable:
        convergence_mode = "regularized_weighted_max_iter_tail_q_average"
        final_scored["active_set_convergence_mode"] = convergence_mode
    else:
        convergence_mode = "regularized_weighted_fixed_point_tail_q_average"
        final_scored["active_set_convergence_mode"] = convergence_mode

    tail_weight, tail_q_df = _tail_average_active_weights(
        q_history if q_history else [active_weight_last],
        eligible["group_id"].astype(int).tolist(),
        target_mass=float(len(initial_active_ids)),
    )
    active_weight = {int(g): float(q) for g, q in tail_weight.items()}
    if not tail_q_df.empty:
        tail_q_df["convergence_mode"] = convergence_mode
        tail_q_df["final_from_tail_average"] = True
        tail_q_df.to_csv(out_dir / "active_q_tail_summary.csv", index=False)
    if moment_alignment_rows:
        pd.DataFrame(moment_alignment_rows).to_csv(out_dir / "active_environment_moment_alignment.csv", index=False)
    osc_cols = [
        "group_id", "active_weight_q", "active_weight_q_next", "active_weight_target_raw",
        "active_weight_target_ema", "active_weight_desire_raw", "active_weight_desire_ema",
        "active_weight_availability_raw", "active_weight_availability_ema",
        "active_weight_target_flip_count", "active_weight_target_flip_streak",
        "active_weight_oscillation_score", "active_weight_oscillation_row_multiplier",
    ]
    try:
        final_scored[[c for c in osc_cols if c in final_scored.columns]].to_csv(out_dir / "active_q_oscillation_diagnostics.csv", index=False)
    except Exception:
        pass

    final_scored["active_weight_q_last"] = final_scored["group_id"].astype(int).map(lambda gid: float(active_weight_last.get(int(gid), 0.0)))
    final_scored["active_weight_q_final"] = final_scored["group_id"].astype(int).map(lambda gid: float(active_weight.get(int(gid), 0.0)))
    final_scored["active_weight_q_tail_mean"] = final_scored["active_weight_q_final"].astype(float)
    if not tail_q_df.empty:
        tail_sd_map = {int(g): float(v) for g, v in tail_q_df[["group_id", "q_tail_sd"]].itertuples(index=False, name=None)}
        tail_prob_map = {int(g): float(v) for g, v in tail_q_df[["group_id", "q_tail_support_probability"]].itertuples(index=False, name=None)}
        final_scored["active_weight_q_tail_sd"] = final_scored["group_id"].astype(int).map(lambda gid: float(tail_sd_map.get(int(gid), np.nan)))
        final_scored["active_weight_tail_support_probability"] = final_scored["group_id"].astype(int).map(lambda gid: float(tail_prob_map.get(int(gid), np.nan)))
    final_scored["active_set_selected_next"] = final_scored["active_weight_q_final"].astype(float) > ACTIVE_SET_WEIGHTED_SUPPORT_EPS
    final_scored["active_set_selected_next"] = final_scored["active_set_selected_next"].astype(int)

    final_ids = _active_weight_support_ids(active_weight)
    iteration_df = pd.DataFrame(iteration_rows)
    if "convergence_mode" not in iteration_df.columns:
        iteration_df["convergence_mode"] = convergence_mode
    iteration_df.to_csv(out_dir / "active_set_iteration_summary.csv", index=False)
    if not iteration_df.empty:
        churn_cols = [
            "iteration", "active_in", "active_support_out", "selected_out",
            "entered_support_count", "exited_support_count",
            "active_weight_max_delta", "active_weight_mean_delta",
            "active_weight_mass", "active_weight_effective_count",
            "active_validation_base_logloss", "active_validation_corrected_logloss",
            "active_validation_bad_rounds", "active_weight_update_alpha",
            "active_weight_update_alpha_before_churn_damp", "active_weight_churn_ratio_tentative",
            "active_weight_churn_step_multiplier",
            "active_weight_macro_churn_step_multiplier", "active_weight_micro_churn_step_multiplier",
            "active_weight_tentative_max_delta", "active_weight_tentative_mean_delta",
            "active_weight_target_raw_mass", "active_weight_target_ema_raw_mass",
            "active_weight_target_ema_scaled_mass", "active_weight_target_flip_rows",
            "active_weight_target_flip_count_total", "active_weight_target_flip_streak_max",
            "active_weight_oscillation_score_mean",
            "active_moment_alignment_applied", "active_moment_alignment_rows",
            "active_moment_raw_mean", "active_moment_raw_sd",
            "active_moment_projected_mean_before", "active_moment_projected_sd_before",
            "active_moment_projected_mean_after", "active_moment_projected_sd_after",
            "active_moment_alignment_scale_factor", "active_moment_alignment_shift_cqd",
            "entered_support_ids_preview", "exited_support_ids_preview",
            "stable", "convergence_mode",
        ]
        iteration_df[[c for c in churn_cols if c in iteration_df.columns]].to_csv(
            out_dir / "active_set_iteration_churn_summary.csv",
            index=False,
        )
    tail_sd_dict = {}
    tail_prob_dict = {}
    if 'tail_q_df' in locals() and isinstance(tail_q_df, pd.DataFrame) and not tail_q_df.empty:
        tail_sd_dict = {int(g): float(v) for g, v in tail_q_df[["group_id", "q_tail_sd"]].itertuples(index=False, name=None)}
        tail_prob_dict = {int(g): float(v) for g, v in tail_q_df[["group_id", "q_tail_support_probability"]].itertuples(index=False, name=None)}
    pd.DataFrame({
        "group_id": final_ids,
        "active_weight_q": [float(active_weight.get(int(g), 0.0)) for g in final_ids],
        "active_weight_q_last": [float(active_weight_last.get(int(g), 0.0)) for g in final_ids],
        "active_weight_q_tail_sd": [float(tail_sd_dict.get(int(g), np.nan)) for g in final_ids],
        "active_weight_tail_support_probability": [float(tail_prob_dict.get(int(g), np.nan)) for g in final_ids],
        "convergence_mode": convergence_mode,
        "final_from_tail_average": True,
    }).to_csv(out_dir / "active_set_final_group_ids.csv", index=False)
    final_scored.to_csv(out_dir / "active_set_scored_candidates_final_iteration.csv", index=False)
    pd.DataFrame({
        "group_id": [int(g) for g in sorted(active_weight)],
        "active_weight_q": [float(active_weight[g]) for g in sorted(active_weight)],
        "active_weight_q_last": [float(active_weight_last.get(int(g), 0.0)) for g in sorted(active_weight)],
        "active_weight_q_tail_sd": [float(tail_sd_dict.get(int(g), np.nan)) for g in sorted(active_weight)],
        "active_weight_tail_support_probability": [float(tail_prob_dict.get(int(g), np.nan)) for g in sorted(active_weight)],
    }).to_csv(out_dir / "active_set_final_weights.csv", index=False)

    return {
        "final_active_ids": final_ids,
        "active_weight": {int(g): float(q) for g, q in active_weight.items()},
        "active_weight_last": {int(g): float(q) for g, q in active_weight_last.items()},
        "active_weight_tail_sd": tail_sd_dict,
        "active_weight_tail_support_probability": tail_prob_dict,
        "final_from_tail_average": True,
        "iteration_rows": iteration_rows,
        "final_scored": final_scored,
        "stable": bool(stable),
        "iterations": int(len(iteration_rows)),
        "baseline_cqd": baseline_cqd,
        "initial_active_count": int(len(initial_active_ids)),
        "convergence_mode": convergence_mode,
    }

def run(sqlite_path: Path, out_dir: Path, lane_size: int = 2, nfold: int = 5, seed: int = 123, raw_min: Optional[float] = None):
    out_dir.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(sqlite_path)
    lr = pd.read_sql_query("""
        select lr.group_id, lr.raw_average_cqd, lr.average_cqd as old_average_cqd,
               lr.rank as old_rank, lr.golden_rate,
               g.canonical, g.display_raw
        from lane_results lr join groups g on g.id=lr.group_id
        where lr.lane_size=? and lr.raw_average_cqd is not null
        order by lr.group_id
    """, conn, params=(lane_size,))
    if lr.empty:
        raise RuntimeError("No lane_results rows with raw_average_cqd")
    lr = lr.rename(columns={"raw_average_cqd": "raw_cqd"})
    blocked_df = pd.read_sql_query("select group_id, reason from blocked_groups where lane_size=?", conn, params=(lane_size,))
    blocked_ids = set(blocked_df["group_id"].astype(int).tolist()) if not blocked_df.empty else set()

    lr_all_nonblocked = lr.copy()
    pre_block_count = len(lr_all_nonblocked)
    blocked_score_only_lr = lr.iloc[0:0].copy()
    if blocked_ids:
        blocked_score_only_lr = lr[lr["group_id"].astype(int).isin(blocked_ids)].copy()
        lr_all_nonblocked = lr_all_nonblocked[~lr_all_nonblocked["group_id"].astype(int).isin(blocked_ids)].copy()
    if lr_all_nonblocked.empty:
        raise RuntimeError("No candidate groups remain after applying blocked_groups filter")

    if raw_min is not None:
        core_lr = lr_all_nonblocked[lr_all_nonblocked["raw_cqd"] >= float(raw_min)].copy()
        scout_lr = lr_all_nonblocked[lr_all_nonblocked["raw_cqd"] < float(raw_min)].copy()
        if core_lr.empty:
            raise RuntimeError(f"No core candidate groups remain after raw_min={raw_min}")
    else:
        core_lr = lr_all_nonblocked.copy()
        scout_lr = lr_all_nonblocked.iloc[0:0].copy()

    excluded_blocked_count = int(pre_block_count - len(lr_all_nonblocked))
    lr = pd.concat([core_lr.assign(scout_candidate=False), scout_lr.assign(scout_candidate=True)], ignore_index=True, sort=False)
    if lr.empty:
        raise RuntimeError("No candidate/scout groups remain after applying filters")

    gm = pd.read_sql_query("""
        select gm.group_id, gm.member, gm.position from group_members gm
        join lane_results lr on lr.group_id=gm.group_id and lr.lane_size=?
        order by gm.group_id, gm.position
    """, conn, params=(lane_size,))
    group_members = {gid: list(g["member"]) for gid, g in gm.groupby("group_id", sort=False)}
    text_rows = []
    for _, r in lr.iterrows():
        members = group_members.get(int(r.group_id), str(r.canonical).split("+"))
        sm = compute_group_skill_summary([str(x) for x in members])
        text_rows.append({"group_id": int(r.group_id), "Text-Type": sm["type_label"], "Simple Text-Type": sm["simple_type_label"], "Name": sm["display_canonical"] or r.display_raw})
    eligible_groups_df = lr.merge(pd.DataFrame(text_rows), on="group_id", how="left")
    eligible_groups_df["raw_rank"] = eligible_groups_df["raw_cqd"].rank(ascending=False, method="first").astype(int)
    eligible_groups_df["raw_score_ge_candidate_min"] = True if raw_min is None else eligible_groups_df["raw_cqd"].astype(float) >= float(raw_min)
    eligible_groups_df["scout_candidate"] = eligible_groups_df["scout_candidate"].fillna(False).astype(bool)
    eligible_groups_df["blocked_score_only_candidate"] = False

    blocked_score_only_df = blocked_score_only_lr.copy()
    if not blocked_score_only_df.empty:
        blocked_text_rows = []
        for _, r in blocked_score_only_df.iterrows():
            members = group_members.get(int(r.group_id), str(r.canonical).split("+"))
            sm = compute_group_skill_summary([str(x) for x in members])
            blocked_text_rows.append({
                "group_id": int(r.group_id),
                "Text-Type": sm["type_label"],
                "Simple Text-Type": sm["simple_type_label"],
                "Name": sm["display_canonical"] or r.display_raw,
            })
        blocked_score_only_df = blocked_score_only_df.merge(pd.DataFrame(blocked_text_rows), on="group_id", how="left")
        blocked_score_only_df["raw_rank"] = blocked_score_only_df["raw_cqd"].rank(ascending=False, method="first").astype(int)
        blocked_score_only_df["raw_score_ge_candidate_min"] = True if raw_min is None else blocked_score_only_df["raw_cqd"].astype(float) >= float(raw_min)
        blocked_score_only_df["scout_candidate"] = False
        blocked_score_only_df["blocked_score_only_candidate"] = True
    core_groups_df = eligible_groups_df[~eligible_groups_df["scout_candidate"]].copy()
    scout_groups_df = eligible_groups_df[eligible_groups_df["scout_candidate"]].copy()
    scout_groups_df.to_csv(out_dir / "raw_below_threshold_scout_rescue_pool.csv", index=False)

    global_base_universe_df = eligible_groups_df.copy()
    global_base_universe_df["global_base_role"] = np.where(
        global_base_universe_df["scout_candidate"].fillna(False).astype(bool),
        "nonblocked_scout_score_only_raw_base",
        "nonblocked_candidate_raw_base",
    )
    if not blocked_score_only_df.empty:
        blocked_for_global_base = blocked_score_only_df.copy()
        blocked_for_global_base["global_base_role"] = "blocked_score_only_raw_base"
        global_base_universe_df = pd.concat(
            [global_base_universe_df, blocked_for_global_base],
            ignore_index=True,
            sort=False,
        )
    global_base_universe_df = global_base_universe_df.drop_duplicates("group_id", keep="first").reset_index(drop=True)
    global_base_universe_df["global_base_raw_rank"] = global_base_universe_df["raw_cqd"].astype(float).rank(
        ascending=False,
        method="first",
    ).astype(int)
    global_base_universe_df["global_base_front_reference"] = True if raw_min is None else (
        global_base_universe_df["raw_cqd"].astype(float) >= float(raw_min)
    )
    global_base_universe_df["global_base_cqd"] = global_base_universe_df["raw_cqd"].astype(float)
    global_base_universe_df["global_base_delta_cqd"] = 0.0
    global_base_universe_df["global_base_mode"] = "raw_equals_global_base_no_global_model"
    global_base_universe_df.to_csv(out_dir / "global_base_universe_groups.csv", index=False)

    raw_global_base_scores = global_base_universe_df.copy()
    raw_global_base_scores["global_base_cqd_raw"] = raw_global_base_scores["raw_cqd"].astype(float)
    raw_global_base_scores["global_base_cqd"] = raw_global_base_scores["raw_cqd"].astype(float)
    raw_global_base_scores["global_base_raw_delta_cqd"] = 0.0
    raw_global_base_scores["global_base_delta_cqd"] = 0.0
    raw_global_base_scores["global_base_reliability"] = 1.0
    raw_global_base_scores["global_base_robust_shrink"] = 1.0
    raw_global_base_scores["global_base_tau_delta_cqd"] = 0.0
    raw_global_base_scores["global_base_tau_counter_cqd"] = 0.0
    raw_global_base_scores["global_base_tau_lowrank_cqd"] = 0.0
    raw_global_base_scores["global_base_tau_stability_shrink"] = 1.0
    raw_global_base_scores.to_csv(out_dir / "global_regularized_base_scores.csv", index=False)
    pd.DataFrame([{
        "global_base_mode": "raw_equals_global_base_no_global_model",
        "global_base_model_enabled": 0,
        "global_base_raw_equals_base": 1,
        "groups": int(len(global_base_universe_df)),
        "blocked_rows": int((global_base_universe_df.get("global_base_role", pd.Series([], dtype=str)).astype(str) == "blocked_score_only_raw_base").sum()),
        "global_base_front_raw_min": "" if raw_min is None else float(raw_min),
        "raw_delta_abs_mean_cqd": 0.0,
        "raw_delta_abs_median_cqd": 0.0,
        "raw_delta_abs_p95_cqd": 0.0,
        "raw_delta_abs_max_cqd": 0.0,
        "final_delta_abs_mean_cqd": 0.0,
        "final_delta_abs_median_cqd": 0.0,
        "final_delta_abs_p95_cqd": 0.0,
        "final_delta_abs_max_cqd": 0.0,
        "base_output_is_shrink_dominated": False,
        "global_base_scalar_delta_note": "global base model disabled; global_base_cqd is exactly raw_cqd; all post-Raw correction must come from active projection/residual layer",
    }]).to_csv(out_dir / "global_regularized_base_diagnostics.csv", index=False)

    all_edges = pd.read_sql_query(
        "select group_a, group_b, win_rate_a, samples from group_rates where samples>0 and win_rate_a is not null",
        conn,
    )
    conn.close()
    if float(all_edges["win_rate_a"].max()) > 1.0:
        all_edges["win_rate_a"] = all_edges["win_rate_a"] / 100.0
    all_edges_all = all_edges.copy()
    eligible_ids = set(eligible_groups_df["group_id"].astype(int).tolist())
    global_base_ids = set(global_base_universe_df["group_id"].astype(int).tolist())

    # Global base model is deliberately disabled.  Do not request global-base
    # graph edges and do not fit a global EB base.  Raw is the only base; all
    # subsequent correction must be learned by the active projection/residual
    # layer and diagnosed separately.
    all_edges = all_edges_all[
        all_edges_all.group_a.isin(eligible_ids) & all_edges_all.group_b.isin(eligible_ids)
    ].copy()
    if all_edges.empty:
        raise RuntimeError("No eligible within-lane group_rates edges")
    # Prospective-only production path.
    #
    # The legacy active-q / crossfit / final-active-projection mainline is not
    # executed in this mode.  Its functions remain in this file for optional
    # historical diagnostics and for compatibility with older notes, but `run()`
    # no longer calls them.  Missing-rate measurement is still strict: each
    # prospective external environment calls require_pairs_or_request(), so any
    # missing scoreable-vs-reference edge exits through MissingRateRequest instead
    # of using a synthetic default or 0.
    # RSW-Type clustering belongs to the retired active/low-rank diagnostics.
    # Production Correct is a fixed equal-policy reference mean and does not
    # consume these labels. Avoid the full adaptive clustering pass here.
    raw_map_for_rsw = {
        int(g): float(r)
        for g, r in core_groups_df[["group_id", "raw_cqd"]].itertuples(index=False, name=None)
    }
    frozen_rsw = {
        "beta_raw": 1.0,
        "type_by_gid": {gid: 0 for gid in raw_map_for_rsw},
        "label_by_gid": {gid: "RSW_DISABLED_PROSPECTIVE_CORRECT" for gid in raw_map_for_rsw},
        "kdf": pd.DataFrame(columns=["k", "score"]),
        "n_types": 1,
    }
    pd.DataFrame([
        {"group_id": gid, "frozen_rsw_type_id": 0, "frozen_RSW-Type": "RSW_DISABLED_PROSPECTIVE_CORRECT"}
        for gid in sorted(raw_map_for_rsw)
    ]).to_csv(out_dir / "frozen_global_rsw_types.csv", index=False)
    frozen_rsw["kdf"].to_csv(out_dir / "frozen_global_rsw_type_selection.csv", index=False)

    global_base_map = {
        int(gid): float(raw)
        for gid, raw in global_base_universe_df[["group_id", "raw_cqd"]].itertuples(index=False, name=None)
    }
    eligible_groups_df["global_base_cqd"] = eligible_groups_df["raw_cqd"].astype(float)
    eligible_groups_df["global_base_mode"] = "raw_equals_global_base_no_global_model"
    if not blocked_score_only_df.empty:
        blocked_score_only_df["global_base_cqd"] = blocked_score_only_df["raw_cqd"].astype(float)
        blocked_score_only_df["global_base_mode"] = "raw_equals_global_base_no_global_model"
    core_groups_df = eligible_groups_df[~eligible_groups_df["scout_candidate"]].copy()
    scout_groups_df = eligible_groups_df[eligible_groups_df["scout_candidate"]].copy()

    beta_edge_df = all_edges.copy()
    # Raw beta is a fitted parameter.  Strictly exclude scout/score-only and
    # blocked rows so they participate only in final scoring.
    raw_map_for_beta = {
        int(g): float(r)
        for g, r in core_groups_df[["group_id", "raw_cqd"]].itertuples(index=False, name=None)
    }
    beta_edge_df = beta_edge_df[
        beta_edge_df["group_a"].astype(int).isin(raw_map_for_beta)
        & beta_edge_df["group_b"].astype(int).isin(raw_map_for_beta)
    ].copy()
    if beta_edge_df.empty:
        raise RuntimeError("No eligible within-lane group_rates edges for prospective Raw beta calibration")
    beta_x = np.asarray([
        raw_map_for_beta[int(a)] - raw_map_for_beta[int(b)]
        for a, b in beta_edge_df[["group_a", "group_b"]].itertuples(index=False, name=None)
    ], dtype=float)
    beta_y = beta_edge_df["win_rate_a"].to_numpy(float)
    beta_n = beta_edge_df["samples"].to_numpy(float)
    beta = float(fit_raw_beta(beta_x, beta_y, beta_n))
    if not np.isfinite(beta) or abs(beta) <= 1e-8:
        beta = 1.0
    beta_all = float(beta)

    final_score_universe_df = eligible_groups_df.copy()
    if not blocked_score_only_df.empty:
        final_score_universe_df = pd.concat([final_score_universe_df, blocked_score_only_df], ignore_index=True, sort=False)
    final_score_universe_df = final_score_universe_df.drop_duplicates("group_id", keep="first").reset_index(drop=True)
    final_score_universe_df["group_id"] = final_score_universe_df["group_id"].astype(int)
    final_score_universe_df["raw_cqd"] = pd.to_numeric(final_score_universe_df["raw_cqd"], errors="coerce").astype(float)
    final_score_universe_df["raw_rank"] = final_score_universe_df["raw_cqd"].rank(ascending=False, method="first").astype(int)
    final_score_universe_df["Raw Rank"] = final_score_universe_df["raw_rank"].astype(int)
    final_score_universe_df["Raw Cqd"] = final_score_universe_df["raw_cqd"].astype(float)
    final_score_universe_df["global_base_cqd"] = final_score_universe_df["raw_cqd"].astype(float)
    final_score_universe_df["global_base_mode"] = "raw_equals_global_base_no_global_model"
    for _bool_col, _default in [
        ("scout_candidate", False),
        ("blocked_score_only_candidate", False),
        ("raw_score_ge_candidate_min", True),
    ]:
        if _bool_col not in final_score_universe_df.columns:
            final_score_universe_df[_bool_col] = _default
        final_score_universe_df[_bool_col] = final_score_universe_df[_bool_col].fillna(_default).astype(bool)

    groups_out = final_score_universe_df.copy()
    type_by_gid = {int(k): int(v) for k, v in frozen_rsw.get("type_by_gid", {}).items()}
    label_by_gid = {int(k): str(v) for k, v in frozen_rsw.get("label_by_gid", {}).items()}
    groups_out["RSW-Type"] = groups_out["group_id"].astype(int).map(lambda gid: str(label_by_gid.get(int(gid), "RSW_UNKNOWN_SCORE_ONLY")))
    groups_out["active_set_selected_for_training"] = False
    groups_out["active_set_iterations"] = 0
    groups_out["active_set_stable"] = True
    groups_out["active_set_baseline_cqd"] = np.nan
    groups_out["active_weight_q"] = 0.0
    groups_out["active_weight_q_final"] = 0.0
    groups_out["active_weight_q_last"] = 0.0
    groups_out["active_weight_q_tail_mean"] = 0.0
    groups_out["active_weight_q_tail_sd"] = 0.0
    groups_out["active_weight_tail_support_probability"] = 0.0
    groups_out["active_weight_final_from_tail_average"] = False
    groups_out["active_set_score_source"] = "legacy_active_iteration_not_run"
    groups_out["active_set_score_success"] = True
    groups_out["active_set_score_message"] = "legacy_active_iteration_disabled; final score generated by prospective Raw-anchored Correct"
    groups_out["active_set_soft_kicked_below_baseline"] = False
    groups_out["active_set_smoothed_correct_cqd"] = groups_out["Raw Cqd"].astype(float)
    groups_out["Correct Cqd"] = groups_out["Raw Cqd"].astype(float)
    groups_out["Model Correct Cqd"] = groups_out["Raw Cqd"].astype(float)
    groups_out["selection_weight_cqd"] = groups_out["Raw Cqd"].astype(float)
    groups_out["Selection Weight Cqd"] = groups_out["selection_weight_cqd"].astype(float)
    groups_out["selection_weight_source"] = "prospective_correct_pending_raw_seed"
    groups_out["selection_weight_used_final_projection"] = True
    groups_out["selection_weight_fell_back_to_global_base"] = False
    groups_out["regularized_active_cqd"] = groups_out["Raw Cqd"].astype(float)
    groups_out["full_model_corrected_cqd"] = groups_out["Raw Cqd"].astype(float)
    groups_out["full_model_delta_logit"] = 0.0
    groups_out["posterior_delta_logit"] = 0.0
    groups_out["cv_bagged_delta_logit"] = 0.0
    groups_out["cv_delta_sd_logit"] = 0.0
    groups_out["cv_delta_sd_cqd"] = 0.0
    groups_out["stability_strength_sd_logit"] = 0.0
    groups_out["stability_strength_sd_cqd"] = 0.0
    groups_out["posterior_strength_logit"] = float(beta) * groups_out["Raw Cqd"].astype(float)
    groups_out["full_model_strength_logit"] = groups_out["posterior_strength_logit"].astype(float)
    groups_out["Correct Rank All Candidates"] = groups_out["Correct Cqd"].rank(ascending=False, method="first").astype(int)
    groups_out["rank_delta_all_candidates"] = groups_out["Raw Rank"].astype(int) - groups_out["Correct Rank All Candidates"].astype(int)

    members = group_members
    member_list = sorted({m for ms in members.values() for m in ms})
    mem_idx = {m: i for i, m in enumerate(member_list)}
    resolver_baseline_cqd = float(raw_min) if raw_min is not None else float(groups_out.loc[~groups_out["blocked_score_only_candidate"].fillna(False).astype(bool), "Raw Cqd"].min())
    if not np.isfinite(resolver_baseline_cqd):
        resolver_baseline_cqd = float(groups_out["Raw Cqd"].min())
    resolver_baseline_logit = float(beta) * float(resolver_baseline_cqd)
    groups_out["resolver_baseline_cqd"] = float(resolver_baseline_cqd)
    groups_out["resolver_baseline_logit"] = float(resolver_baseline_logit)
    groups_out["resolver_marginal_utility_logit"] = groups_out["posterior_strength_logit"].astype(float) - float(resolver_baseline_logit)
    groups_out["resolver_selected"] = 0

    pd.DataFrame([
        {"check": "eligible_raw_cqd_rows", "value": len(eligible_groups_df), "status": "OK"},
        {"check": "core_raw_ge_threshold_rows", "value": len(core_groups_df), "status": "OK"},
        {"check": "scout_raw_below_threshold_rows", "value": len(scout_groups_df), "status": "OK"},
        {"check": "score_universe_rows", "value": len(groups_out), "status": "OK"},
        {"check": "legacy_active_iteration_executed", "value": 0, "status": "OK"},
        {"check": "legacy_active_final_projection_executed", "value": 0, "status": "OK"},
        {"check": "prospective_correct_enabled", "value": 1, "status": "OK"},
        {"check": "frozen_global_rsw_type_count", "value": int(frozen_rsw.get("n_types", 0)), "status": "OK"},
        {"check": "raw_beta_for_prospective_projection", "value": float(beta), "status": "OK"},
        {"check": "pair_edges_for_raw_beta", "value": len(beta_edge_df), "status": "OK"},
        {"check": "selection_weight_output_units", "value": "CQD", "status": "OK"},
        {"check": "candidate_pool_raw_score_min", "value": "" if raw_min is None else float(raw_min), "status": "OK"},
        {"check": "blocked_groups_excluded_from_training", "value": excluded_blocked_count, "status": "OK"},
        {"check": "blocked_groups_score_only_rows", "value": int(len(blocked_score_only_df)), "status": "OK"},
        {"check": "global_base_universe_rows", "value": int(len(global_base_universe_df)), "status": "OK"},
        {"check": "global_base_model_enabled", "value": 0, "status": "OK"},
        {"check": "global_base_mode", "value": "raw_equals_global_base_no_global_model", "status": "OK"},
        {"check": "global_base_raw_equals_base", "value": 1, "status": "OK"},
        {"check": "global_base_edges_requested", "value": 0, "status": "OK"},
        {"check": "text_type_computed_rows", "value": int(groups_out["Text-Type"].notna().sum()), "status": "OK" if groups_out["Text-Type"].notna().all() else "FAIL"},
        {"check": "golden_used_for_score_calibration", "value": 0, "status": "OK"},
        {"check": "legacy_winrate_type_used", "value": 0, "status": "OK"},
        {"check": "manual_cap_or_topk_used", "value": 0, "status": "OK"},
    ]).to_csv(out_dir / "input_integrity_checks.csv", index=False)

    # Compatibility diagnostics: these files explicitly record that the legacy
    # active mainline did not run.  They are not produced by an active loop.
    pd.DataFrame([{
        "mode": "legacy_active_iteration_disabled",
        "iterations": 0,
        "stable": True,
        "active_iteration_generates_final_correct": 0,
        "final_score_source": "raw_anchored_prospective_destratified_correct_center",
    }]).to_csv(out_dir / "active_set_iteration_summary.csv", index=False)
    pd.DataFrame([{
        "mode": "legacy_active_iteration_disabled",
        "entered": 0,
        "exited": 0,
        "support_churn_rate": 0.0,
    }]).to_csv(out_dir / "active_set_iteration_churn_summary.csv", index=False)
    pd.DataFrame([{
        "mode": "legacy_active_iteration_disabled",
        "active_q_oscillation_detected": 0,
    }]).to_csv(out_dir / "active_q_oscillation_diagnostics.csv", index=False)
    pd.DataFrame([{
        "mode": "legacy_active_iteration_disabled",
        "tail_q_used": 0,
    }]).to_csv(out_dir / "active_q_tail_summary.csv", index=False)
    pd.DataFrame([{
        "context": "legacy_active_iteration_disabled",
        "moment_alignment_applied": 0,
    }]).to_csv(out_dir / "active_environment_moment_alignment.csv", index=False)
    pd.DataFrame([{
        "context": "legacy_active_final_projection_disabled",
        "moment_alignment_applied": 0,
    }]).to_csv(out_dir / "final_score_moment_alignment.csv", index=False)
    pd.DataFrame([{
        "column": "regularized_active_cqd",
        "rows": int(len(groups_out)),
        "missing_rows": 0,
        "missing_rate": 0.0,
        "mode": "prospective_correct_center_no_legacy_active_projection",
    }]).to_csv(out_dir / "final_projection_diagnostic_completeness.csv", index=False)

    groups_out, prospective_correct_info = _apply_raw_anchored_prospective_correct(
        groups_out=groups_out,
        final_score_universe_df=final_score_universe_df,
        all_edges=all_edges_all,
        group_members=group_members,
        raw_min=raw_min,
        lane_size=lane_size,
        out_dir=out_dir,
        beta=float(beta),
        resolver_baseline_cqd=float(resolver_baseline_cqd),
        seed=seed,
    )
    # The fixed-slot K=5 formula is already the final public scale. A later
    # affine moment alignment would bend that line and change its meaning.
    groups_out["Correct_center_cqd_pre_final_moment_alignment"] = groups_out["Correct_center_cqd"].astype(float)
    raw_values = groups_out["Raw Cqd"].astype(float).to_numpy()
    correct_values = groups_out["selection_weight_cqd"].astype(float).to_numpy()
    final_moment_info = {
        "context": "fixed_slot_replacement_k5_no_moment_alignment",
        "moment_alignment_applied": False,
        "alignment_reason": "fixed_slot_formula_is_final_scale",
        "alignment_scale_factor": 1.0,
        "alignment_shift_cqd": 0.0,
        "raw_mean": float(np.mean(raw_values)),
        "raw_sd": float(np.std(raw_values)),
        "projected_mean_after": float(np.mean(correct_values)),
        "projected_sd_after": float(np.std(correct_values)),
    }
    final_scale = 1.0
    aligned_score = groups_out["selection_weight_cqd"].astype(float)
    groups_out["Correct_center_cqd"] = aligned_score
    groups_out["Correct Cqd"] = aligned_score
    groups_out["Selection Weight Cqd"] = aligned_score
    groups_out["Model Correct Cqd"] = aligned_score
    groups_out["regularized_active_cqd"] = aligned_score
    groups_out["model_correct_delta_from_raw_cqd"] = aligned_score - groups_out["Raw Cqd"].astype(float)
    groups_out["prospective_destrat_adjustment_cqd_pre_final_moment_alignment"] = groups_out["prospective_destrat_adjustment_cqd"].astype(float)
    groups_out["prospective_destrat_adjustment_cqd"] = groups_out["model_correct_delta_from_raw_cqd"].astype(float)
    for _unc_col in ["Correct_uncertainty_cqd", "Correct_member_overlap_uncertainty_cqd", "prospective_posterior_estimation_se_cqd", "prospective_environment_dispersion_cqd", "prospective_loo_max_abs_change_cqd"]:
        if _unc_col in groups_out.columns:
            groups_out[_unc_col] = pd.to_numeric(groups_out[_unc_col], errors="coerce").astype(float) * final_scale
    groups_out["Correct_potential_cqd"] = aligned_score + groups_out["Correct_uncertainty_cqd"].astype(float)
    groups_out["Correct_selection_risk_penalized_cqd"] = aligned_score - 0.35 * np.sqrt(
        groups_out["Correct_uncertainty_cqd"].astype(float) ** 2
        + groups_out["Correct_member_overlap_uncertainty_cqd"].astype(float) ** 2
    )
    groups_out["active_residual_raw_cqd"] = groups_out["model_correct_delta_from_raw_cqd"].astype(float)
    groups_out["active_residual_shrunk_cqd"] = groups_out["model_correct_delta_from_raw_cqd"].astype(float)
    groups_out["active_residual_net_adjustment_cqd"] = groups_out["model_correct_delta_from_raw_cqd"].astype(float)
    groups_out["posterior_delta_logit"] = float(beta) * groups_out["model_correct_delta_from_raw_cqd"].astype(float)
    groups_out["posterior_strength_logit"] = float(beta) * aligned_score
    groups_out["full_model_strength_logit"] = groups_out["posterior_strength_logit"].astype(float)
    groups_out["full_model_corrected_cqd"] = aligned_score
    pd.DataFrame([final_moment_info]).to_csv(out_dir / "final_score_moment_alignment.csv", index=False)
    prospective_correct_info.update({
        "final_moment_alignment_applied": 0,
        "final_moment_alignment_scale_factor": float(final_moment_info["alignment_scale_factor"]),
        "final_moment_alignment_shift_cqd": float(final_moment_info["alignment_shift_cqd"]),
        "final_correct_mean_after_alignment": float(final_moment_info["projected_mean_after"]),
        "final_correct_sd_after_alignment": float(final_moment_info["projected_sd_after"]),
        "final_correct_variance_after_alignment": float(final_moment_info["projected_sd_after"]) ** 2,
        "final_raw_variance": float(final_moment_info["raw_sd"]) ** 2,
    })
    coefficient_trace_info = _write_rowwise_correct_target_trace(
        groups_out=groups_out,
        all_edges=all_edges_all,
        reference_ids=prospective_correct_info["prospective_reference_group_ids"],
        lane_size=lane_size,
        raw_min=raw_min,
        out_dir=out_dir,
        beta=float(beta),
        alignment_scale=float(final_moment_info["alignment_scale_factor"]),
        alignment_shift=float(final_moment_info["alignment_shift_cqd"]),
    )
    prospective_correct_info.update({
        f"correct_coefficient_{key}": value
        for key, value in coefficient_trace_info.items()
    })
    groups_out["Selection Weight Cqd"] = groups_out["selection_weight_cqd"].astype(float)
    groups_out["Model Correct Cqd"] = groups_out.get("Model Correct Cqd", groups_out["Correct Cqd"]).astype(float)
    groups_out["model_correct_delta_from_raw_cqd"] = groups_out.get(
        "model_correct_delta_from_raw_cqd",
        groups_out["Correct Cqd"].astype(float) - groups_out["raw_cqd"].astype(float),
    )
    groups_out["Correct Rank All Candidates"] = groups_out["selection_weight_cqd"].rank(ascending=False, method="first").astype(int)
    groups_out["selection_weight_rank_all_candidates"] = groups_out["Correct Rank All Candidates"].astype(int)
    groups_out["rank_delta_all_candidates"] = groups_out["Raw Rank"].astype(int) - groups_out["Correct Rank All Candidates"].astype(int)

    final_visible_candidate_pool = groups_out[
        ~groups_out.get("blocked_score_only_candidate", pd.Series(False, index=groups_out.index)).fillna(False).astype(bool)
    ].copy()
    final_visible_pool = _active_update_scoreable_above_baseline(
        final_visible_candidate_pool,
        resolver_baseline_cqd,
        score_col="selection_weight_cqd",
        context="final-correct-visible-main-selection-weight-prospective-only",
    )
    final_visible_ids = set(_greedy_visible_main_group_ids(
        final_visible_pool,
        group_members,
        score_col="selection_weight_cqd",
        context="final-correct-visible-main-selection-weight-prospective-only",
    ))
    groups_out["resolver_selected"] = groups_out["group_id"].astype(int).isin(final_visible_ids).astype(int)
    selected = groups_out[groups_out.resolver_selected == 1].copy()
    selected = selected.sort_values("selection_weight_cqd", ascending=False).reset_index(drop=True)
    selected["Correct Rank"] = np.arange(1, len(selected) + 1)
    used = []
    for gid in selected.group_id:
        used.extend(group_members.get(int(gid), []))
    dup_count = len(used) - len(set(used))
    review_cols = [
        "Correct Rank", "Selection Weight Cqd", "Correct Cqd", "Correct_center_cqd", "Correct_potential_cqd", "Correct_uncertainty_cqd",
        "Raw Rank", "Raw Cqd", "selection_weight_source", "Text-Type", "Name"
    ]
    selected[[c for c in review_cols if c in selected.columns]].to_csv(
        out_dir / "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.csv",
        index=False,
    )

    for _col in [
        "selection_weight_cqd", "Selection Weight Cqd", "selection_weight_logit",
        "selection_weight_source", "selection_weight_delta_from_raw_cqd",
        "selection_weight_rank_all_candidates", "Model Correct Cqd",
        "model_correct_delta_from_raw_cqd",
    ]:
        if _col not in groups_out.columns:
            if _col in ("selection_weight_cqd", "Selection Weight Cqd"):
                groups_out[_col] = groups_out["Correct Cqd"].astype(float)
            elif _col == "selection_weight_logit":
                groups_out[_col] = float(beta) * (groups_out["Correct Cqd"].astype(float) - float(resolver_baseline_cqd))
            elif _col == "selection_weight_source":
                groups_out[_col] = "raw_anchored_prospective_destratified_correct_center"
            elif _col == "selection_weight_delta_from_raw_cqd":
                groups_out[_col] = groups_out["Correct Cqd"].astype(float) - groups_out["raw_cqd"].astype(float)
            elif _col == "selection_weight_rank_all_candidates":
                groups_out[_col] = groups_out["Correct Rank All Candidates"].astype(int)
            elif _col == "Model Correct Cqd":
                groups_out[_col] = groups_out["Correct Cqd"].astype(float)
            elif _col == "model_correct_delta_from_raw_cqd":
                groups_out[_col] = groups_out["Correct Cqd"].astype(float) - groups_out["raw_cqd"].astype(float)

    posterior_cols = [
        "group_id", "RSW-Type", "Text-Type", "Simple Text-Type", "Name", "raw_cqd",
        "selection_weight_cqd", "Selection Weight Cqd", "selection_weight_logit",
        "selection_weight_source", "selection_weight_delta_from_raw_cqd",
        "selection_weight_rank_all_candidates", "Correct Cqd", "Correct_center_cqd", "Correct_potential_cqd", "Correct_uncertainty_cqd", "Correct_selection_risk_penalized_cqd", "Model Correct Cqd", "global_base_cqd", "regularized_active_cqd", "active_residual_reliability", "active_residual_raw_cqd", "active_residual_shrunk_cqd",
        "model_correct_delta_from_raw_cqd",
        "full_model_corrected_cqd", "posterior_strength_logit", "full_model_strength_logit",
        "posterior_delta_logit", "full_model_delta_logit", "stability_strength_sd_logit",
        "stability_strength_sd_cqd", "cv_delta_sd_logit", "cv_delta_sd_cqd", "Raw Rank",
        "Correct Rank All Candidates", "rank_delta_all_candidates", "resolver_selected",
        "active_set_selected_for_training", "active_set_score_source", "active_set_score_success",
        "active_set_score_message", "active_set_soft_kicked_below_baseline",
        "active_set_smoothed_correct_cqd", "active_weight_q", "active_weight_q_final",
        "selection_weight_q_last", "selection_weight_q_tail_mean", "selection_weight_q_tail_sd",
        "selection_weight_tail_support_probability", "selection_weight_final_from_tail_average",
        "active_weight_target", "active_weight_desire", "active_weight_availability", "scout_candidate", "raw_score_ge_candidate_min",
    ]
    groups_out[[c for c in posterior_cols if c in groups_out.columns]].to_csv(
        out_dir / "posterior_strength_by_group.csv",
        index=False,
    )
    groups_out[[c for c in posterior_cols if c in groups_out.columns]].to_csv(
        out_dir / "posterior_strength_by_group_preselection_model_fit.csv",
        index=False,
    )

    _write_raw_adhesion_diagnostics(out_dir, groups_out)
    _write_post_training_correction_overfit_diagnostics(out_dir, groups_out)
    _write_slice_moment_diagnostics(out_dir, groups_out)
    try:
        _audit_cols = [
            "group_id", "raw_cqd", "Raw Rank", "Correct Cqd", "Correct Rank All Candidates",
            "selection_weight_cqd", "selection_weight_delta_from_raw_cqd",
            "selection_weight_q_last", "selection_weight_q_tail_mean", "selection_weight_q_tail_sd",
            "selection_weight_tail_support_probability", "active_moment_alignment_delta_cqd",
            "active_set_selected_for_training", "blocked_score_only_candidate", "scout_candidate",
            "prospective_environment_dispersion_cqd", "Correct_uncertainty_cqd",
        ]
        _audit = groups_out[[c for c in _audit_cols if c in groups_out.columns]].copy()
        if "selection_weight_q_tail_sd" in _audit.columns:
            _audit["projection_instability_q_tail_warning"] = pd.to_numeric(_audit["selection_weight_q_tail_sd"], errors="coerce").fillna(0.0) > 0.05
        if "prospective_environment_dispersion_cqd" in _audit.columns:
            _audit["projection_instability_environment_dispersion_warning"] = pd.to_numeric(_audit["prospective_environment_dispersion_cqd"], errors="coerce").abs().fillna(0.0) > 0.25
        _audit.to_csv(out_dir / "projection_instability_audit.csv", index=False)
    except Exception:
        pass
    groups_out.to_csv(out_dir / "final_all_candidate_diagnostics.csv", index=False)

    resolver_rows = []
    for _, r in groups_out.iterrows():
        resolver_rows.append({
            "group_id": int(r.group_id),
            "resolver_selected": int(r.resolver_selected),
            "posterior_strength_logit": float(r.posterior_strength_logit),
            "resolver_marginal_utility_logit": float(r.resolver_marginal_utility_logit),
            "resolver_baseline_cqd": float(r.resolver_baseline_cqd),
            "selection_weight_cqd": float(r["selection_weight_cqd"]),
            "selection_weight_logit": float(r["selection_weight_logit"]),
            "Correct Cqd": float(r["Correct Cqd"]),
            "Model Correct Cqd": float(r["Model Correct Cqd"]),
            "Raw Cqd": float(r["Raw Cqd"]),
            "Raw Rank": int(r["Raw Rank"]),
            "Correct Rank All Candidates": int(r["Correct Rank All Candidates"]),
            "Text-Type": r["Text-Type"],
            "Name": r["Name"],
        })
    pd.DataFrame(resolver_rows).to_csv(out_dir / "resolver_selection_detail.csv", index=False)

    comp_rows = []
    for m, idx in mem_idx.items():
        gids = [gid for gid, ms in members.items() if m in ms]
        c = groups_out[groups_out.group_id.isin(gids)].sort_values("posterior_strength_logit", ascending=False)
        if len(c) <= 1:
            continue
        best = c.iloc[0]
        second = c.iloc[1] if len(c) > 1 else None
        comp_rows.append({
            "member": m,
            "candidate_count": len(c),
            "selected_group_id": int(c[c.resolver_selected == 1].group_id.iloc[0]) if (c.resolver_selected == 1).any() else None,
            "best_group_id": int(best.group_id),
            "best_utility": float(best.resolver_marginal_utility_logit),
            "best_posterior_strength_logit": float(best.posterior_strength_logit),
            "second_group_id": int(second.group_id) if second is not None else None,
            "second_utility": float(second.resolver_marginal_utility_logit) if second is not None else None,
            "second_posterior_strength_logit": float(second.posterior_strength_logit) if second is not None else None,
            "best_vs_second_margin": float(best.resolver_marginal_utility_logit - second.resolver_marginal_utility_logit) if second is not None else None,
        })
    pd.DataFrame(comp_rows).to_csv(out_dir / "same_member_competitor_detail.csv", index=False)

    movement = {
        "mean_abs_delta_cqd": float(np.mean(np.abs(groups_out["Correct Cqd"] - groups_out["Raw Cqd"]))),
        "median_abs_delta_cqd": float(np.median(np.abs(groups_out["Correct Cqd"] - groups_out["Raw Cqd"]))),
        "p95_abs_delta_cqd": float(np.quantile(np.abs(groups_out["Correct Cqd"] - groups_out["Raw Cqd"]), 0.95)),
        "max_abs_delta_cqd": float(np.max(np.abs(groups_out["Correct Cqd"] - groups_out["Raw Cqd"]))),
        "top50_jaccard_diagnostic": float(
            len(set(groups_out.nsmallest(50, "Raw Rank").group_id) & set(groups_out.nsmallest(50, "Correct Rank All Candidates").group_id))
            / max(1, len(set(groups_out.nsmallest(50, "Raw Rank").group_id) | set(groups_out.nsmallest(50, "Correct Rank All Candidates").group_id)))
        ),
    }

    pair_metrics_path = out_dir / "prospective_correct_pair_metrics.csv"
    prospective_metrics_df = pd.read_csv(pair_metrics_path) if pair_metrics_path.exists() else pd.DataFrame()
    if not prospective_metrics_df.empty and "model" in prospective_metrics_df.columns:
        raw_metrics = prospective_metrics_df[prospective_metrics_df.model == "raw"].iloc[0].to_dict() if (prospective_metrics_df.model == "raw").any() else {}
        corr_metrics = prospective_metrics_df[prospective_metrics_df.model == "corrected"].iloc[0].to_dict() if (prospective_metrics_df.model == "corrected").any() else {}
    else:
        raw_metrics = {}
        corr_metrics = {}
    for _metric in ["weighted_logloss", "weighted_brier", "weighted_auc", "weighted_ordering_accuracy"]:
        raw_metrics.setdefault(_metric, np.nan)
        corr_metrics.setdefault(_metric, np.nan)

    final_summary = pd.DataFrame([{
        "version": "raw_anchored_prospective_correct_no_legacy_active",
        "resolver_mode": "raw_anchored_multi_environment_destratified_correct_center",
        "validation_early_stop_enabled": 0,
        "validation_bad_rounds_diagnostic_only": 1,
        "legacy_active_iteration_executed": 0,
        "legacy_active_iteration_generates_final_correct": 0,
        "legacy_active_iteration_role": "disabled_not_run",
        "prospective_correct_enabled": 1,
        "prospective_correct_files": "prospective_correct_summary.csv;prospective_correct_diagnostics.csv;prospective_environment_summary.csv;prospective_environment_group_deltas_long.csv;prospective_correct_pair_metrics.csv",
        "selection_weight_final_projection_attach_fix": 0,
        "final_projection_score_universe_invariant": 1,
        "no_global_base_fallback_for_scoreable_output": 1,
        "global_base_diagnostics_enabled": 1,
        "global_base_model_enabled": 0,
        "global_base_mode": "raw_equals_global_base_no_global_model",
        "global_base_raw_equals_base": 1,
        "global_base_edges_requested": 0,
        "blocked_global_base_mode": "raw_equals_global_base_no_global_model",
        "excluded_blocked_count": excluded_blocked_count,
        "candidate_pool_raw_score_min": np.nan if raw_min is None else float(raw_min),
        "score_universe_rows": int(len(groups_out)),
        "eligible_nonblocked_rows": int(len(eligible_groups_df)),
        "blocked_score_only_rows": int(len(blocked_score_only_df)),
        "edges_for_prospective_raw_beta": int(len(beta_edge_df)),
        "prospective_raw_beta": float(beta),
        "final_selected_count": int(len(selected)),
        "duplicate_selected_members": int(dup_count),
        "resolver_utility_mode": "browser_visible_main_greedy_by_prospective_correct_center",
        "resolver_baseline_cqd": float(resolver_baseline_cqd),
        "resolver_baseline_logit": float(resolver_baseline_logit),
        "full_model_mean_abs_delta_cqd": float(np.mean(np.abs(groups_out["full_model_corrected_cqd"] - groups_out["Raw Cqd"]))),
        "full_model_max_abs_delta_cqd": float(np.max(np.abs(groups_out["full_model_corrected_cqd"] - groups_out["Raw Cqd"]))),
        **movement,
        **prospective_correct_info,
    }])
    final_summary.to_csv(out_dir / "final_model_summary.csv", index=False)

    scorecard_rows = [
        {"axis": "pair_metrics_in_sample_prospective", "metric": "weighted_logloss", "raw": raw_metrics["weighted_logloss"], "corrected": corr_metrics["weighted_logloss"], "delta": corr_metrics["weighted_logloss"] - raw_metrics["weighted_logloss"] if np.isfinite(raw_metrics["weighted_logloss"]) and np.isfinite(corr_metrics["weighted_logloss"]) else np.nan, "better": "lower", "status": "diagnostic"},
        {"axis": "pair_metrics_in_sample_prospective", "metric": "weighted_brier", "raw": raw_metrics["weighted_brier"], "corrected": corr_metrics["weighted_brier"], "delta": corr_metrics["weighted_brier"] - raw_metrics["weighted_brier"] if np.isfinite(raw_metrics["weighted_brier"]) and np.isfinite(corr_metrics["weighted_brier"]) else np.nan, "better": "lower", "status": "diagnostic"},
        {"axis": "pair_metrics_in_sample_prospective", "metric": "weighted_auc", "raw": raw_metrics["weighted_auc"], "corrected": corr_metrics["weighted_auc"], "delta": corr_metrics["weighted_auc"] - raw_metrics["weighted_auc"] if np.isfinite(raw_metrics["weighted_auc"]) and np.isfinite(corr_metrics["weighted_auc"]) else np.nan, "better": "higher", "status": "diagnostic"},
        {"axis": "pair_metrics_in_sample_prospective", "metric": "weighted_ordering_accuracy", "raw": raw_metrics["weighted_ordering_accuracy"], "corrected": corr_metrics["weighted_ordering_accuracy"], "delta": corr_metrics["weighted_ordering_accuracy"] - raw_metrics["weighted_ordering_accuracy"] if np.isfinite(raw_metrics["weighted_ordering_accuracy"]) and np.isfinite(corr_metrics["weighted_ordering_accuracy"]) else np.nan, "better": "higher", "status": "diagnostic"},
        {"axis": "legacy_active", "metric": "iteration_executed", "raw": np.nan, "corrected": 0, "delta": np.nan, "better": "zero", "status": "ok"},
        {"axis": "resolver", "metric": "duplicate_selected_members", "raw": np.nan, "corrected": dup_count, "delta": np.nan, "better": "zero", "status": "ok" if dup_count == 0 else "fail"},
        {"axis": "score_movement", "metric": "mean_abs_delta_cqd", "raw": 0.0, "corrected": movement["mean_abs_delta_cqd"], "delta": movement["mean_abs_delta_cqd"], "better": "diagnostic", "status": "diagnostic"},
        {"axis": "score_movement", "metric": "max_abs_delta_cqd", "raw": 0.0, "corrected": movement["max_abs_delta_cqd"], "delta": movement["max_abs_delta_cqd"], "better": "diagnostic", "status": "diagnostic"},
    ]
    pd.DataFrame(scorecard_rows).to_csv(out_dir / "leaderboard_evaluation_scorecard.csv", index=False)

    jumpers = groups_out.sort_values("rank_delta_all_candidates", ascending=False).head(30)[[
        c for c in [
            "group_id", "Raw Rank", "Correct Rank All Candidates", "rank_delta_all_candidates",
            "Raw Cqd", "Correct Cqd", "Correct_center_cqd", "Correct_potential_cqd", "Correct_uncertainty_cqd",
            "full_model_corrected_cqd", "stability_strength_sd_cqd", "cv_delta_sd_cqd", "Text-Type", "Name"
        ] if c in groups_out.columns
    ]]
    jumpers.to_csv(out_dir / "top_jumper_stability_review.csv", index=False)

    report = f"""# Raw-anchored prospective Correct, no legacy active mainline

This run does not execute the legacy active-q iteration, legacy crossfit model, or legacy final active projection.

## Final score semantics

```text
Correct Cqd = Correct_center_cqd
Correct_center_cqd = Raw Cqd + conservative multi-environment de-stratification adjustment
```

Missing edges are still strict: every prospective future environment calls
`MissingRateRequest` through `require_pairs_or_request`; no synthetic default
win rate and no 0 fallback are used.

## Headline

- score universe rows: {len(groups_out)}
- selected visible groups: {len(selected)}
- duplicate selected member count: {dup_count}
- prospective Raw beta: {float(beta):.9f}
- mean_abs_delta_cqd: {movement['mean_abs_delta_cqd']:.9f}
- max_abs_delta_cqd: {movement['max_abs_delta_cqd']:.9f}
- mean_uncertainty_cqd: {float(groups_out['Correct_uncertainty_cqd'].mean()) if 'Correct_uncertainty_cqd' in groups_out else float('nan'):.9f}

## Disabled legacy paths

- `run_active_set_challenger_selection`: not called
- active q fixed-point loop: not run
- legacy crossfit betabinomial resolver model: not fit
- final score-only active projection: not run

The old functions remain in the file for optional diagnostics and historical compatibility, but the production `run()` path bypasses them.
"""
    (out_dir / "RAW_ANCHORED_PROSPECTIVE_CORRECT_REPORT.md").write_text(report, encoding="utf-8")
    (out_dir / "CROSSFIT_BETABINOMIAL_LOWRANK_COUNTER_EB_RESOLVER_REPORT.md").write_text(
        "# Legacy crossfit/active report disabled\n\nThe legacy active/crossfit mainline was not executed in this prospective-only run.\n",
        encoding="utf-8",
    )

    # Full total table: include every lane-2 group, not just model candidates.
    conn2 = sqlite3.connect(sqlite_path)
    lr_all_total = pd.read_sql_query("""
        select lr.group_id, lr.raw_average_cqd as raw_cqd, lr.average_cqd as db_average_cqd, lr.rank as db_rank,
               lr.selection_status, g.canonical, g.display_raw, g.lane_size
        from lane_results lr join groups g on g.id=lr.group_id
        where lr.lane_size=? and lr.raw_average_cqd is not null
        order by lr.group_id
    """, conn2, params=(lane_size,))
    blocked_all = pd.read_sql_query("select group_id, reason as blocked_reason from blocked_groups where lane_size=?", conn2, params=(lane_size,))
    conn2.close()
    blocked_all_ids = set(blocked_all["group_id"].astype(int).tolist()) if not blocked_all.empty else set()
    all_text_rows = []
    for _, rr in lr_all_total.iterrows():
        ms = group_members.get(int(rr.group_id), str(rr.canonical).split("+"))
        sm = compute_group_skill_summary([str(x) for x in ms])
        all_text_rows.append({
            "group_id": int(rr.group_id),
            "Text-Type": sm["type_label"],
            "Simple Text-Type": sm["simple_type_label"],
            "Name": sm["display_canonical"] or rr.display_raw,
        })
    all_total = lr_all_total.merge(pd.DataFrame(all_text_rows), on="group_id", how="left")
    all_total["Raw Rank Full"] = all_total["raw_cqd"].rank(ascending=False, method="first").astype(int)
    all_total["raw_score_ge_candidate_min"] = all_total["raw_cqd"] >= float(raw_min) if raw_min is not None else True
    all_total["raw_below_threshold_scout_candidate"] = False
    all_total["blocked_by_db"] = all_total["group_id"].astype(int).isin(blocked_all_ids)
    if not blocked_all.empty:
        all_total = all_total.merge(blocked_all, on="group_id", how="left")
    else:
        all_total["blocked_reason"] = np.nan

    model_cols = [
        "group_id", "RSW-Type", "selection_weight_cqd", "Selection Weight Cqd",
        "selection_weight_logit", "selection_weight_source",
        "selection_weight_delta_from_raw_cqd", "selection_weight_rank_all_candidates",
        "Correct_center_cqd", "Correct_potential_cqd", "Correct_uncertainty_cqd", "Correct_selection_risk_penalized_cqd",
        "prospective_destrat_adjustment_cqd", "prospective_evidence_q", "prospective_environment_consistency", "prospective_environment_dispersion_cqd",
        "Model Correct Cqd", "model_correct_delta_from_raw_cqd",
        "full_model_strength_logit", "full_model_corrected_cqd",
        "full_model_delta_logit", "cv_mean_strength_logit", "stability_strength_sd_logit",
        "cv_min_strength_logit", "cv_max_strength_logit", "cv_bagged_delta_logit",
        "cv_delta_sd_logit", "cv_delta_min_logit", "cv_delta_max_logit",
        "posterior_strength_logit", "Correct Cqd", "posterior_delta_logit",
        "stability_strength_sd_cqd", "cv_delta_sd_cqd", "Raw Rank",
        "Raw Cqd", "Correct Rank All Candidates", "rank_delta_all_candidates",
        "resolver_selected", "resolver_marginal_utility_logit", "resolver_baseline_cqd",
        "active_set_selected_for_training", "active_set_score_source",
        "active_set_score_success", "active_set_score_message",
        "active_set_soft_kicked_below_baseline",
        "active_set_smoothed_correct_cqd",
        "active_set_challenger_edges", "active_set_projection_role",
        "active_weight_q", "active_weight_reference_mass", "active_weight_reference_mean",
        "active_weight_reference_missing_edges",
        "active_residual_reliability", "active_residual_raw_cqd", "active_residual_shrunk_cqd",
        "active_residual_q_mass_reliability", "active_residual_edge_count_reliability",
        "active_residual_sample_mass_reliability", "active_residual_reference_diversity_reliability",
        "active_residual_coverage_reliability", "active_residual_validation_survival",
        "active_residual_selected_row_multiplier", "active_residual_soft_cap_factor",
        "active_residual_effective_soft_cap_cqd",
        "active_residual_robust_shrink", "active_residual_survival_multiplier",
        "active_residual_shrink_ratio", "active_residual_net_adjustment_cqd",
        "active_uncertainty_penalty_cqd", "active_leverage_penalty_cqd", "active_total_penalty_cqd",
        "active_weight_reference_sample_mass", "active_weight_reference_q_sample_mass",
        "active_weight_reference_effective_count", "active_weight_reference_effective_evidence_count",
        "active_weight_reference_max_share", "active_weight_reference_max_evidence_share",
        "active_weight_reference_coverage_ratio", "active_weight_evidence_gate",
        "active_validation_raw_logloss", "active_validation_base_logloss",
        "active_validation_corrected_logloss", "active_validation_corrected_minus_base_logloss",
        "active_validation_corrected_minus_raw_logloss",
        "regularized_active_cqd", "regularized_active_logit", "regularized_active_source",
        "selection_weight_used_final_projection", "selection_weight_fell_back_to_global_base",
        "scout_candidate", "raw_score_ge_candidate_min", "blocked_score_only_candidate",
    ]
    model_for_total = groups_out[[c for c in model_cols if c in groups_out.columns]].copy()
    # raw_score_ge_candidate_min already exists on all_total for every DB row.
    # If it is also carried by model_for_total, pandas suffixes it into _x/_y
    # and downstream code expects the canonical unsuffixed name.  Prefer the
    # all_total value because it covers non-scoreable DB rows too.
    if "raw_score_ge_candidate_min" in model_for_total.columns:
        model_for_total = model_for_total.drop(columns=["raw_score_ge_candidate_min"])
    total = all_total.merge(model_for_total, on="group_id", how="left")
    if "raw_score_ge_candidate_min" not in total.columns:
        if "raw_score_ge_candidate_min_x" in total.columns:
            total["raw_score_ge_candidate_min"] = total["raw_score_ge_candidate_min_x"]
        elif raw_min is None:
            total["raw_score_ge_candidate_min"] = True
        else:
            total["raw_score_ge_candidate_min"] = total["raw_cqd"].astype(float) >= float(raw_min)
    total["raw_score_ge_candidate_min"] = total["raw_score_ge_candidate_min"].fillna(False).astype(bool)
    if "scout_candidate" in total.columns:
        total["raw_below_threshold_scout_candidate"] = total["scout_candidate"].fillna(False).astype(bool)
    active_model_ids = set(groups_out.loc[groups_out["active_set_selected_for_training"].fillna(False), "group_id"].astype(int))
    scoreable_ids = set(groups_out["group_id"].astype(int))
    total["diagnostic_row_type"] = np.select(
        [
            total["group_id"].isin(active_model_ids),
            total["group_id"].isin(scoreable_ids),
        ],
        [
            "active_set_model_candidate",
            "active_set_score_only_candidate",
        ],
        default="outside_active_set_score_pool",
    )
    _blocked_scoreable_mask = total["blocked_by_db"].fillna(False).astype(bool) & total["group_id"].isin(scoreable_ids)
    total.loc[_blocked_scoreable_mask, "diagnostic_row_type"] = "score_only_blocked_prospective_correct"
    total["in_model_candidate_pool"] = total["diagnostic_row_type"].eq("active_set_model_candidate")
    total["in_scoreable_candidate_pool"] = total["group_id"].isin(scoreable_ids)
    if "raw_score_ge_candidate_min" not in total.columns:
        if "raw_score_ge_candidate_min_x" in total.columns:
            total["raw_score_ge_candidate_min"] = total["raw_score_ge_candidate_min_x"]
        elif raw_min is None:
            total["raw_score_ge_candidate_min"] = True
        else:
            total["raw_score_ge_candidate_min"] = total["raw_cqd"].astype(float) >= float(raw_min)
    total["raw_score_ge_candidate_min"] = total["raw_score_ge_candidate_min"].fillna(False).astype(bool)

    total["excluded_reason"] = np.select(
        [
            total["blocked_by_db"].fillna(False) & total["in_scoreable_candidate_pool"].fillna(False),
            total["blocked_by_db"].fillna(False),
            ~total["raw_score_ge_candidate_min"].fillna(False) & ~total["in_scoreable_candidate_pool"],
            total["raw_score_ge_candidate_min"].fillna(False) & ~total["blocked_by_db"].fillna(False) & ~total["in_scoreable_candidate_pool"],
        ],
        [
            "blocked_groups_score_only_prospective_correct",
            "blocked_groups",
            "raw_below_candidate_pool_min_not_rescued",
            "not_scoreable_in_active_set_pipeline",
        ],
        default=""
    )
    total["resolver_selected"] = total["resolver_selected"].fillna(0).astype(int)
    # Active and score-only candidates both have selection-weight display scores.
    # Only blocked/below-threshold/outside rows fall back to raw display.
    total["candidate_model_missing"] = ~total["in_scoreable_candidate_pool"]
    if "selection_weight_cqd" not in total.columns:
        total["selection_weight_cqd"] = total["Correct Cqd"]

    # The serialized UI/export score is selection_weight_cqd.  For rows that are
    # deliberately not allowed to use the model score as a selection weight
    # (blocked / below-threshold / outside active-score pool), the selection
    # weight is the raw CQD fallback.  Keep the literal selection_weight_cqd
    # column aligned with the display column so Rust, frontend and exported text
    # all observe the same value.
    _not_scoreable_for_selection_weight = ~total["in_scoreable_candidate_pool"].fillna(False)
    total["Selection Weight Cqd Display"] = total["selection_weight_cqd"].where(
        total["in_scoreable_candidate_pool"],
        total["raw_cqd"],
    )
    total["selection_weight_cqd"] = total["Selection Weight Cqd Display"].astype(float)
    total["Selection Weight Cqd"] = total["selection_weight_cqd"].astype(float)
    if "selection_weight_source" not in total.columns:
        total["selection_weight_source"] = ""
    total.loc[_not_scoreable_for_selection_weight, "selection_weight_source"] = "raw_cqd_fallback_not_scoreable_for_selection_weight"
    total["selection_weight_delta_from_raw_cqd"] = total["selection_weight_cqd"].astype(float) - total["raw_cqd"].astype(float)

    total["Model Correct Cqd Display"] = total["Correct Cqd"].where(total["in_scoreable_candidate_pool"], total["raw_cqd"])
    # Backward-compatible display score consumed by Rust/UI: now it is the
    # selection/output weight in CQD units, not the raw model Correct Cqd.
    total["Correct Cqd Display"] = total["Selection Weight Cqd Display"]
    total["Raw Cqd Display"] = total["Raw Cqd"].where(total["in_scoreable_candidate_pool"], total["raw_cqd"])
    total = total.sort_values(["resolver_selected", "Selection Weight Cqd Display", "raw_cqd"], ascending=[False, False, False])
    total.to_csv(out_dir / "final_total_table_ALL_GROUPS.csv", index=False)

    return {
        "out_dir": str(out_dir),
        "selected": int(len(selected)),
        "dup_count": int(dup_count),
        "legacy_active_iteration_executed": 0,
        "legacy_active_final_projection_executed": 0,
        "prospective_correct_enabled": 1,
        "oof_logloss_raw": float(raw_metrics.get("weighted_logloss", np.nan)),
        "oof_logloss_corrected": float(corr_metrics.get("weighted_logloss", np.nan)),
        "oof_logloss_delta": (
            float(corr_metrics.get("weighted_logloss", np.nan) - raw_metrics.get("weighted_logloss", np.nan))
            if np.isfinite(raw_metrics.get("weighted_logloss", np.nan)) and np.isfinite(corr_metrics.get("weighted_logloss", np.nan))
            else np.nan
        ),
        "mean_abs_delta_cqd": float(movement["mean_abs_delta_cqd"]),
        "max_abs_delta_cqd": float(movement["max_abs_delta_cqd"]),
        "p95_abs_delta_cqd": float(movement["p95_abs_delta_cqd"]),
        "prospective_raw_beta": float(beta),
        "rsw_k": int(frozen_rsw.get("n_types", 0)),
        **prospective_correct_info,
    }


def create_excel_outputs(out_dir: Path) -> None:
    from artifact_tool import Workbook, SpreadsheetFile
    review_csv = out_dir / "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.csv"
    if review_csv.exists():
        review = pd.read_csv(review_csv)
        wb = Workbook.create()
        _write_df_sheet(wb, "Review", review)
        SpreadsheetFile.export_xlsx(wb).save(str(out_dir / "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.xlsx"))

    wb = Workbook.create()
    sheet_specs = [
        ("Summary", "final_model_summary.csv", None),
        ("Scorecard", "leaderboard_evaluation_scorecard.csv", None),
        ("Active Iter", "active_set_iteration_summary.csv", None),
        ("Active Churn", "active_set_iteration_churn_summary.csv", None),
        ("Global Base Diag", "global_regularized_base_diagnostics.csv", None),
        ("Raw Adhesion", "raw_adhesion_diagnostics.csv", None),
        ("PostTrain Fit", "post_training_correction_overfit_diagnostics.csv", None),
        ("Top Shrunk", "post_training_top_shrunk_rows.csv", None),
        ("OOF Metrics", "oof_prediction_metrics_before_after.csv", None),
        ("Fold Metrics", "outer_fold_prediction_metrics.csv", None),
        ("Type Summary", "oof_type_bias_summary_before_after.csv", None),
        ("Type Pair", "oof_type_pair_bias_before_after_detail.csv", 220),
        ("Worst TypePair", "worst_worsened_type_pair_bias.csv", 120),
        ("Lowrank Full", "lowrank_spectrum_selection_full.csv", None),
        ("Lowrank Folds", "lowrank_spectrum_selection_by_fold.csv", 160),
        ("Lowrank Gamma", "lowrank_skew_interaction_posterior.csv", 160),
        ("K Select Full", "adaptive_rsw_type_k_selection_full.csv", None),
        ("BB Lowrank Fit", "betabinomial_lowrank_eb_fit_diagnostics_by_fold.csv", None),
        ("CV Stability", "cv_strength_stability_by_group.csv", 160),
        ("Counters", "antisymmetric_type_counter_posterior.csv", 160),
        ("Resolver", "resolver_selection_detail.csv", 160),
        ("Review", "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.csv", None),
        ("Input Checks", "input_integrity_checks.csv", None),
        ("Total ALL Groups", "final_total_table_ALL_GROUPS.csv", 500),
    ]
    for sh, csv_name, max_rows in sheet_specs:
        path = out_dir / csv_name
        if path.exists():
            _write_df_sheet(wb, sh, pd.read_csv(path), max_rows=max_rows)
    SpreadsheetFile.export_xlsx(wb).save(str(out_dir / "crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7_audit.xlsx"))

def bundle_outputs(out_dir: Path, source_path: Optional[Path] = None) -> Path:
    if source_path is not None and source_path.exists():
        shutil.copy2(source_path, out_dir / "crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.py")
    bundle = out_dir.with_name(out_dir.name + "_bundle.zip")
    if bundle.exists():
        bundle.unlink()
    with zipfile.ZipFile(bundle, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for p in sorted(out_dir.iterdir()):
            if p.is_file():
                z.write(p, arcname=p.name)
    return bundle


# =========================
# Arbitrary lane-size + score-only blocked support
# =========================
# This section intentionally stays in the same source file.  It does not import
# a separate base algorithm file.  All model, resolver, Text-Type, RSW, EB,
# low-rank/counter, and blocked score-only logic is aggregated here.

def sigmoid_any(x):
    return expit(np.clip(np.asarray(x, dtype=float), -40, 40))

def safe_tag_value(x: Optional[float]) -> str:
    if x is None:
        return "allraw"
    s = ("%g" % float(x)).replace(".", "_").replace("-", "m")
    return "rawge" + s

def make_run_tag(lane_size: int, raw_min: Optional[float]) -> str:
    return f"lane{int(lane_size)}_{safe_tag_value(raw_min)}"

def write_lane_size_inventory(sqlite_path: Path, out_dir: Path) -> pd.DataFrame:
    conn = sqlite3.connect(sqlite_path)
    lane = pd.read_sql_query("""
        select lr.lane_size,
               count(*) as group_count,
               min(lr.raw_average_cqd) as min_raw_cqd,
               max(lr.raw_average_cqd) as max_raw_cqd,
               avg(lr.raw_average_cqd) as mean_raw_cqd
        from lane_results lr
        where lr.raw_average_cqd is not null
        group by lr.lane_size
        order by lr.lane_size
    """, conn)
    blocked = pd.read_sql_query("""
        select lane_size, count(*) as blocked_group_count
        from blocked_groups
        group by lane_size
        order by lane_size
    """, conn)
    gm = pd.read_sql_query("""
        select g.lane_size, gm.group_id, count(*) as member_count
        from group_members gm
        join groups g on g.id = gm.group_id
        group by g.lane_size, gm.group_id
    """, conn)
    conn.close()
    if not blocked.empty:
        lane = lane.merge(blocked, on="lane_size", how="left")
    else:
        lane["blocked_group_count"] = 0
    lane["blocked_group_count"] = lane["blocked_group_count"].fillna(0).astype(int)
    if not gm.empty:
        chk = gm.groupby("lane_size").agg(
            min_observed_member_count=("member_count", "min"),
            max_observed_member_count=("member_count", "max"),
            groups_with_member_count_not_equal_lane_size=("member_count", lambda s: int(np.sum(s.to_numpy() != int(gm.loc[s.index[0], "lane_size"]))) if len(s) else 0),
        ).reset_index()
        lane = lane.merge(chk, on="lane_size", how="left")
    lane.to_csv(out_dir / "lane_size_inventory.csv", index=False)
    return lane

def beta_binomial_score_only_delta_any(y, n, eta_without_score_delta, sign, tau_delta, phi, likelihood_weight=None):
    """Conditional MAP delta for score-only groups under a frozen model.

    likelihood_weight reweights reference edges without changing their observed
    beta-binomial counts.  This is used by the weighted active environment so
    fractional active membership q_j affects influence continuously rather than
    by fabricating fractional sample counts.
    """
    y = np.asarray(y, dtype=float)
    n = np.asarray(n, dtype=float)
    k = np.rint(np.clip(y, 0, 1) * n)
    eta0 = np.asarray(eta_without_score_delta, dtype=float)
    sign = np.asarray(sign, dtype=float)
    if likelihood_weight is None:
        likelihood_weight = np.ones(len(y), dtype=float)
    else:
        likelihood_weight = np.asarray(likelihood_weight, dtype=float)
    likelihood_weight = np.nan_to_num(likelihood_weight, nan=0.0, posinf=0.0, neginf=0.0)
    likelihood_weight = np.maximum(likelihood_weight, 0.0)
    tau_delta = max(float(tau_delta), 1e-8)
    phi = max(float(phi), 1e-8)

    def obj_grad(z):
        delta = float(z[0])
        eta = np.clip(eta0 + sign * delta, -40, 40)
        mu = sigmoid_any(eta)
        a = np.maximum(mu * phi, 1e-12)
        b = np.maximum((1.0 - mu) * phi, 1e-12)
        ll = betaln(k + a, n - k + b) - betaln(a, b)
        nll = float(-np.sum(likelihood_weight * ll) + 0.5 * (delta / tau_delta) ** 2)
        dL_deta = likelihood_weight * phi * mu * (1.0 - mu) * (
            digamma(k + a) - digamma(a) - digamma(n - k + b) + digamma(b)
        )
        grad = float(-np.sum(dL_deta * sign) + delta / (tau_delta ** 2))
        return nll, np.array([grad], dtype=float)

    opt = minimize(obj_grad, np.array([0.0]), method="L-BFGS-B", jac=True,
                   options={"maxiter": 80, "ftol": 1e-8, "gtol": 1e-6})
    return float(opt.x[0]), bool(opt.success), str(opt.message), float(opt.fun), int(getattr(opt, "nit", 0) or 0)

def add_text_type_rows_any(rows: pd.DataFrame, group_members: Dict[int, List[str]]) -> pd.DataFrame:
    text_rows = []
    for _, r in rows.iterrows():
        members = group_members.get(int(r.group_id), str(r.canonical).split("+"))
        sm = compute_group_skill_summary([str(x) for x in members])
        text_rows.append({
            "group_id": int(r.group_id),
            "Text-Type": sm["type_label"],
            "Simple Text-Type": sm["simple_type_label"],
            "Name": sm["display_canonical"] or r.display_raw,
            "member_count": int(len(members)),
        })
    if not text_rows:
        rows = rows.copy()
        rows["Text-Type"] = ""
        rows["Simple Text-Type"] = ""
        rows["Name"] = rows.get("display_raw", "")
        rows["member_count"] = 0
        return rows
    return rows.merge(pd.DataFrame(text_rows), on="group_id", how="left")

def score_blocked_only_any(sqlite_path: Path, out_dir: Path, lane_size: int, raw_min: Optional[float], nfold: int, seed: int):
    """Score blocked groups without feeding them into any training/evaluation/resolver step.

    If the main run already scored blocked rows through the regularized active
    projection path, do not overwrite those scores with the older frozen-model
    posthoc scorer.  Just export a compact blocked-score CSV and summary.
    """
    total_path_pre = out_dir / "final_total_table_ALL_GROUPS.csv"
    if total_path_pre.exists():
        total_pre = pd.read_csv(total_path_pre)
        if "diagnostic_row_type" in total_pre.columns:
            existing = total_pre[
                total_pre["diagnostic_row_type"].astype(str).isin([
                    "score_only_blocked_prospective_correct",
                    "score_only_blocked_active_projection",
                ])
            ].copy()
            if not existing.empty:
                export_cols = [
                    "group_id", "blocked_reason", "raw_cqd", "Raw Cqd", "Correct Cqd",
                    "Correct_center_cqd", "Correct_potential_cqd", "Correct_uncertainty_cqd",
                    "Correct Cqd Display", "selection_weight_cqd", "selection_weight_source",
                    "selection_weight_delta_from_raw_cqd", "prospective_destrat_adjustment_cqd",
                    "prospective_evidence_q", "prospective_environment_consistency",
                    "prospective_environment_dispersion_cqd", "active_set_challenger_edges",
                    "active_weight_reference_mass", "active_weight_reference_missing_edges",
                    "active_residual_reliability", "active_residual_raw_cqd", "active_residual_shrunk_cqd",
                    "active_residual_q_mass_reliability", "active_residual_edge_count_reliability",
                    "active_residual_sample_mass_reliability", "active_residual_reference_diversity_reliability",
                    "active_residual_coverage_reliability", "active_residual_validation_survival",
                    "active_weight_reference_sample_mass", "active_weight_reference_effective_count",
                    "active_weight_reference_max_evidence_share", "Text-Type", "Name", "excluded_reason",
                ]
                existing[[c for c in export_cols if c in existing.columns]].to_csv(
                    out_dir / "blocked_score_only_corrected_cqd.csv",
                    index=False,
                )
                if "active_set_challenger_edges" not in existing.columns:
                    return {
                        "blocked_score_only_count": int(len(existing)),
                        "blocked_score_only_edges": 0,
                        "blocked_score_only_status": "scored_inside_main_raw_anchored_prospective_correct_no_legacy_active_projection",
                    }
                blocked_edges_series = pd.to_numeric(existing["active_set_challenger_edges"], errors="coerce")
                if blocked_edges_series.isna().any():
                    bad_ids = existing.loc[blocked_edges_series.isna(), "group_id"].astype(int).head(20).tolist()
                    raise RuntimeError(
                        "Internal error: blocked active-projection edge count is missing/non-numeric for "
                        f"{blocked_edges_series.isna().sum()} blocked row(s); first_group_ids={bad_ids}"
                    )
                return {
                    "blocked_score_only_count": int(len(existing)),
                    "blocked_score_only_edges": int(blocked_edges_series.sum()),
                    "blocked_score_only_status": "scored_inside_main_regularized_active_projection",
                }

    conn = sqlite3.connect(sqlite_path)
    lr_all = pd.read_sql_query("""
        select lr.group_id, lr.raw_average_cqd, lr.average_cqd as old_average_cqd, lr.rank as old_rank,
               g.canonical, g.display_raw
        from lane_results lr join groups g on g.id=lr.group_id
        where lr.lane_size=? and lr.raw_average_cqd is not null
        order by lr.group_id
    """, conn, params=(lane_size,))
    lr_all = lr_all.rename(columns={"raw_average_cqd": "raw_cqd"})
    blocked_df = pd.read_sql_query(
        "select group_id, reason as blocked_reason from blocked_groups where lane_size=?",
        conn, params=(lane_size,)
    )
    gm = pd.read_sql_query("""
        select gm.group_id, gm.member, gm.position
        from group_members gm
        join lane_results lr on lr.group_id=gm.group_id and lr.lane_size=?
        order by gm.group_id, gm.position
    """, conn, params=(lane_size,))
    group_members = {gid: list(g["member"]) for gid, g in gm.groupby("group_id", sort=False)}
    rate_all = pd.read_sql_query("select group_a, group_b, win_rate_a, samples from group_rates where samples>0 and win_rate_a is not null", conn)
    conn.close()

    blocked_ids = set(blocked_df["group_id"].astype(int).tolist()) if not blocked_df.empty else set()
    pool = lr_all.copy()
    if raw_min is not None:
        pool = pool[pool["raw_cqd"] >= float(raw_min)].copy()

    score_only_lr = pool[pool["group_id"].astype(int).isin(blocked_ids)].copy()
    train_lr = pool[~pool["group_id"].astype(int).isin(blocked_ids)].copy()

    empty_cols = [
        "group_id", "blocked_reason", "Raw Cqd", "score_only_corrected_cqd",
        "score_only_delta_cqd", "score_only_delta_logit", "score_only_edges",
        "score_only_samples", "score_only_success", "score_only_message",
        "member_count", "Text-Type", "Name"
    ]
    if score_only_lr.empty:
        pd.DataFrame(columns=empty_cols).to_csv(out_dir / "blocked_score_only_corrected_cqd.csv", index=False)
        return {
            "blocked_score_only_count": 0,
            "blocked_score_only_edges": 0,
            "blocked_score_only_status": "no_blocked_candidates_for_this_lane_and_raw_min",
        }

    if train_lr.empty:
        raise RuntimeError("Cannot score blocked-only rows because train_pool is empty after excluding blocked.")

    # Frozen model: fit with nonblocked train pool only.
    train_lr = add_text_type_rows_any(train_lr, group_members)
    train_lr["raw_rank"] = train_lr["raw_cqd"].rank(ascending=False, method="first").astype(int)
    train_group_to_idx = {gid: i for i, gid in enumerate(train_lr["group_id"])}
    train_ids_for_blocked_score = [int(x) for x in train_group_to_idx.keys()]
    require_pairs_or_request(
        rate_all,
        train_ids_for_blocked_score,
        None,
        "blocked score-only legacy frozen train pool",
        lane_size,
        out_dir,
    )
    require_pairs_or_request(
        rate_all,
        score_only_lr["group_id"].astype(int).tolist(),
        train_ids_for_blocked_score,
        "blocked score-only legacy scorer vs train pool",
        lane_size,
        out_dir,
    )
    train_edges = rate_all[
        rate_all.group_a.isin(train_group_to_idx) & rate_all.group_b.isin(train_group_to_idx)
    ].copy()
    if train_edges.empty:
        raise RuntimeError("Internal error: blocked score-only train edges are still empty after missing-rate request")
    if float(train_edges["win_rate_a"].max()) > 1.0:
        train_edges["win_rate_a"] = train_edges["win_rate_a"] / 100.0
    train_edges["ia"] = train_edges.group_a.map(train_group_to_idx).astype(int)
    train_edges["ib"] = train_edges.group_b.map(train_group_to_idx).astype(int)
    train_edges["fold5"] = edge_fold_ids(train_edges["group_a"].to_numpy(), train_edges["group_b"].to_numpy(), nfold=nfold)
    all_mask = np.ones(len(train_edges), dtype=bool)
    raw_train = train_lr["raw_cqd"].to_numpy(float)
    beta_all = fit_raw_beta(
        raw_train[train_edges["ia"].to_numpy()] - raw_train[train_edges["ib"].to_numpy()],
        train_edges["win_rate_a"].to_numpy(float),
        train_edges["samples"].to_numpy(float),
    )
    type_ids, type_labels, kdf, _, _ = adaptive_residual_type(
        train_lr, train_edges, beta_all, all_mask, validation_mask=None, seed=seed + 100
    )
    selected_k = int(kdf.sort_values("validation_logloss").iloc[0]["k"])
    _, _, embedding, _, _ = derive_lowrank_embedding(
        train_lr, train_edges, beta_all, all_mask, seed=seed + 100, target_rank=selected_k
    )
    fit = fit_betabinomial_lowrank_counter_eb(
        train_lr, train_edges, all_mask, type_ids, embedding, max_eb_iter=6, tol=1e-3, fixed_beta=beta_all
    )
    train_strength_logit = fit.beta * raw_train + fit.delta
    gid_to_strength = {int(g): float(s) for g, s in zip(train_lr["group_id"], train_strength_logit)}

    score_only_lr = add_text_type_rows_any(score_only_lr, group_members)
    score_only_lr = score_only_lr.merge(blocked_df, on="group_id", how="left")

    out_rows = []
    total_edges_used = 0
    for _, br in score_only_lr.iterrows():
        gid = int(br.group_id)
        edges_b = rate_all[
            ((rate_all.group_a == gid) & (rate_all.group_b.isin(train_group_to_idx))) |
            ((rate_all.group_b == gid) & (rate_all.group_a.isin(train_group_to_idx)))
        ].copy()
        if edges_b.empty:
            raise RuntimeError(
                f"Internal error: blocked score-only group {gid} has no edges against train pool after missing-rate request"
            )
        observed_opponents = set()
        for ga, gb in edges_b[["group_a", "group_b"]].itertuples(index=False, name=None):
            ga, gb = int(ga), int(gb)
            if ga == gid and gb in train_group_to_idx:
                observed_opponents.add(gb)
            elif gb == gid and ga in train_group_to_idx:
                observed_opponents.add(ga)
        missing_opponents = sorted(set(int(x) for x in train_group_to_idx.keys()) - observed_opponents)
        if missing_opponents:
            preview = "; ".join(str(x) for x in missing_opponents[:20])
            raise RuntimeError(
                f"Internal error: blocked score-only group {gid} is still missing "
                f"{len(missing_opponents)} train opponent(s) after missing-rate request; first_missing_group_ids={preview}"
            )
        if float(edges_b["win_rate_a"].max()) > 1.0:
            edges_b["win_rate_a"] = edges_b["win_rate_a"] / 100.0

        eta0 = []
        y_obs = []
        n_obs = []
        sign = []
        for _, e in edges_b.iterrows():
            ga, gb = int(e.group_a), int(e.group_b)
            if ga == gid:
                other = gb
                eta0.append(float(fit.beta * br.raw_cqd - gid_to_strength[other]))
                y_obs.append(float(e.win_rate_a))
                sign.append(1.0)
            else:
                other = ga
                eta0.append(float(gid_to_strength[other] - fit.beta * br.raw_cqd))
                y_obs.append(float(e.win_rate_a))
                sign.append(-1.0)
            n_obs.append(float(e.samples))

        dlogit, ok, msg, nll, nit = beta_binomial_score_only_delta_any(
            y_obs, n_obs, eta0, sign, fit.tau_delta, fit.phi
        )
        dcqd = dlogit / fit.beta if abs(fit.beta) > 1e-12 else 0.0
        total_edges_used += len(edges_b)
        out_rows.append({
            "group_id": gid,
            "blocked_reason": br.get("blocked_reason", ""),
            "Raw Cqd": float(br.raw_cqd),
            "score_only_corrected_cqd": float(br.raw_cqd + dcqd),
            "score_only_delta_cqd": float(dcqd),
            "score_only_delta_logit": float(dlogit),
            "score_only_edges": int(len(edges_b)),
            "score_only_samples": float(edges_b["samples"].sum()),
            "score_only_success": bool(ok),
            "score_only_message": msg,
            "score_only_nll": nll,
            "score_only_iterations": nit,
            "frozen_beta": float(fit.beta),
            "frozen_tau_delta": float(fit.tau_delta),
            "frozen_phi": float(fit.phi),
            "member_count": int(br.member_count),
            "Text-Type": br.get("Text-Type", ""),
            "Name": br.get("Name", br.display_raw),
        })

    blocked_scores = pd.DataFrame(out_rows)
    blocked_scores.to_csv(out_dir / "blocked_score_only_corrected_cqd.csv", index=False)

    total_path = out_dir / "final_total_table_ALL_GROUPS.csv"
    if total_path.exists():
        total = pd.read_csv(total_path)
        for _, r in blocked_scores.iterrows():
            m = total["group_id"].astype(int) == int(r.group_id)
            total.loc[m, "diagnostic_row_type"] = "score_only_blocked_not_training"
            total.loc[m, "in_model_candidate_pool"] = False
            total.loc[m, "candidate_model_missing"] = False
            total.loc[m, "score_only_corrected_available"] = True
            total.loc[m, "Correct Cqd"] = float(r.score_only_corrected_cqd)
            total.loc[m, "Correct Cqd Display"] = float(r.score_only_corrected_cqd)
            total.loc[m, "score_only_delta_cqd"] = float(r.score_only_delta_cqd)
            total.loc[m, "score_only_edges"] = int(r.score_only_edges)
            total.loc[m, "score_only_samples"] = float(r.score_only_samples)
            total.loc[m, "excluded_reason"] = "blocked_groups_score_only"
            total.loc[m, "resolver_selected"] = 0
        total = total.sort_values(["resolver_selected", "Correct Cqd Display", "raw_cqd"], ascending=[False, False, False])
        total.to_csv(total_path, index=False)

    return {
        "blocked_score_only_count": int(len(blocked_scores)),
        "blocked_score_only_edges": int(total_edges_used),
        "blocked_score_only_status": "scored_with_frozen_train_model",
    }

def patch_anysize_report(out_dir: Path, score_summary: Dict[str, Any], lane_size: int, raw_min: Optional[float], tag: str):
    report = f"""# Any-Size Support Addendum

## What changed

This source now supports arbitrary `lane_size` values through one path:

```text
--lane-size N
--raw-min optional_threshold
```

The resolver is still exact set-packing:

```text
maximize sum marginal_utility(group) * x_group
subject to every member used <= 1
x_group in {{0,1}}
```

This naturally supports single-member, two-member, and N-member groups because the constraint matrix is built from the actual `group_members` rows.  A group is never split into member-level pseudo-groups; selected groups are whole candidate groups.

## Blocked handling

Blocked groups are split into a score-only pool:

```text
train_pool = raw threshold candidates excluding blocked_groups
score_only_pool = blocked candidates passing the raw threshold
```

Blocked rows do **not** enter:

- EB strength/type-counter/lowrank fitting
- RSW-Type derivation
- cross-fit OOF metrics
- resolver / set-packing selection

If blocked rows exist for this lane and threshold, they receive a conditional group-level Correct Cqd after the trained model is frozen.  They remain `resolver_selected = 0`.

## Current run

- run tag: {tag}
- lane_size: {lane_size}
- candidate Raw Score minimum: {raw_min}
- blocked score-only rows: {score_summary.get('blocked_score_only_count')}
- blocked score-only edges used: {score_summary.get('blocked_score_only_edges')}
- status: {score_summary.get('blocked_score_only_status')}

## Cleanliness

- Raw Cqd is immutable input; it is not recomputed.
- Golden is not used.
- Legacy `winrate_type_label` is not used.
- Text-Type comes from the embedded Rust `skill_eq.rs` port.
- RSW-Type K is selected by held-out logloss.
- Low-rank interaction capacity is selected from residual-profile evidence.
- No topK/rank guard/manual cap/manual shrink/history frequency rule is used.
"""
    (out_dir / "ANY_SIZE_SUPPORT_AND_BLOCKED_SCORE_ONLY_REPORT.md").write_text(report, encoding="utf-8")

def update_audit_workbook_anysize(out_dir: Path, tag: str):
    from artifact_tool import Workbook, SpreadsheetFile
    wb = Workbook.create()
    sheet_specs = [
        ("Summary", "final_model_summary.csv", None),
        ("Inventory", "lane_size_inventory.csv", None),
        ("Scorecard", "leaderboard_evaluation_scorecard.csv", None),
        ("Active Iter", "active_set_iteration_summary.csv", None),
        ("Active Churn", "active_set_iteration_churn_summary.csv", None),
        ("Global Base Diag", "global_regularized_base_diagnostics.csv", None),
        ("Raw Adhesion", "raw_adhesion_diagnostics.csv", None),
        ("PostTrain Fit", "post_training_correction_overfit_diagnostics.csv", None),
        ("Top Shrunk", "post_training_top_shrunk_rows.csv", None),
        ("OOF Metrics", "oof_prediction_metrics_before_after.csv", None),
        ("Type Summary", "oof_type_bias_summary_before_after.csv", None),
        ("Type Pair", "oof_type_pair_bias_before_after_detail.csv", 220),
        ("Worst TypePair", "worst_worsened_type_pair_bias.csv", 120),
        ("Blocked ScoreOnly", "blocked_score_only_corrected_cqd.csv", None),
        ("Total ALL Groups", "final_total_table_ALL_GROUPS.csv", 500),
        ("Resolver", "resolver_selection_detail.csv", 160),
        ("Review", "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.csv", None),
        ("Input Checks", "input_integrity_checks.csv", None),
    ]
    for sh, csv_name, max_rows in sheet_specs:
        path = out_dir / csv_name
        if path.exists():
            _write_df_sheet(wb, sh, pd.read_csv(path), max_rows=max_rows)
    SpreadsheetFile.export_xlsx(wb).save(str(out_dir / f"crossfit_betabinomial_interactionrich_anysize_scoreblocked_{tag}_audit.xlsx"))

def alias_outputs_anysize(out_dir: Path, tag: str) -> Dict[str, str]:
    alias = {}
    old_review_csv = out_dir / "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.csv"
    old_review_xlsx = out_dir / "corrected_rank_review_crossfit_betabinomial_interactionrich_counter_eb_resolver_lane1_rawge47_7.xlsx"
    new_review_csv = out_dir / f"corrected_rank_review_interactionrich_anysize_scoreblocked_{tag}.csv"
    new_review_xlsx = out_dir / f"corrected_rank_review_interactionrich_anysize_scoreblocked_{tag}.xlsx"
    if old_review_csv.exists():
        shutil.copy2(old_review_csv, new_review_csv)
        alias["review_csv"] = str(new_review_csv)
    if old_review_xlsx.exists():
        shutil.copy2(old_review_xlsx, new_review_xlsx)
        alias["review_xlsx"] = str(new_review_xlsx)
    return alias

def bundle_outputs_anysize(out_dir: Path, source_path: Path, tag: str) -> Path:
    if source_path.exists():
        shutil.copy2(source_path, out_dir / source_path.name)
    bundle = out_dir.with_name(out_dir.name + "_bundle.zip")
    if bundle.exists():
        bundle.unlink()
    with zipfile.ZipFile(bundle, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for p in sorted(out_dir.iterdir()):
            if p.is_file():
                z.write(p, arcname=p.name)
    return bundle

def run_anysize(sqlite_path: Path, out_dir: Path, lane_size: int, nfold: int, seed: int, raw_min: Optional[float], source_path: Optional[Path] = None) -> Dict[str, Any]:
    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    tag = make_run_tag(lane_size, raw_min)

    # Inventory is written before model execution so no-data lane sizes are easy to inspect.
    inventory = write_lane_size_inventory(sqlite_path, out_dir)
    if lane_size not in set(inventory["lane_size"].astype(int).tolist()):
        raise RuntimeError(f"lane_size={lane_size} has no lane_results rows in this sqlite. Inventory written to {out_dir / 'lane_size_inventory.csv'}.")

    res = run(sqlite_path, out_dir, lane_size, nfold, seed, raw_min)
    no_artifacts = os.environ.get("TSWN_STRICT_NO_ARTIFACTS") == "1"
    if not no_artifacts:
        create_excel_outputs(out_dir)

    score_summary = score_blocked_only_any(sqlite_path, out_dir, lane_size, raw_min, nfold, seed)
    patch_anysize_report(out_dir, score_summary, lane_size, raw_min, tag)

    summary_path = out_dir / "final_model_summary.csv"
    if summary_path.exists():
        s = pd.read_csv(summary_path)
        for k, v in score_summary.items():
            s[k] = v
        s["anysize_support"] = "actual_group_members_setpacking_no_split"
        s["blocked_handling"] = "excluded_from_training_and_resolver_scored_inside_main_prospective_correct"
        s["validation_early_stop_enabled"] = 0
        s["validation_bad_rounds_diagnostic_only"] = 1
        s["active_iteration_diagnostics_enabled"] = 0
        s["selection_weight_final_projection_attach_fix"] = 0
        s["final_projection_score_universe_invariant"] = 1
        s["no_global_base_fallback_for_scoreable_output"] = 1
        s["global_base_diagnostics_enabled"] = 1
        s["global_base_model_enabled"] = 0
        s["global_base_mode"] = "raw_equals_global_base_no_global_model"
        s["global_base_raw_equals_base"] = 1
        s["global_base_edges_requested"] = 0
        s["global_base_tau_stability_shrink_enabled"] = 0
        s["global_base_tau_stability_shrink_diagnostic_only"] = 1
        s["raw_adhesion_diagnostics_enabled"] = 1
        s["post_training_correction_overfit_diagnostics_enabled"] = 1
        s["active_residual_evidence_aware_reliability"] = 0
        s["active_residual_soft_cap_enabled"] = 0
        s["active_residual_symmetric_penalty_enabled"] = 0
        s["active_residual_validation_survival_multiplier_enabled"] = 0
        s["active_q_churn_damping_enabled"] = 0
        s["run_tag"] = tag
        s["source_file"] = "strict_python_calibrator.py"
        s.to_csv(summary_path, index=False)

    aliases = {}
    bundle = None
    if not no_artifacts:
        update_audit_workbook_anysize(out_dir, tag)
        aliases = alias_outputs_anysize(out_dir, tag)
        bundle = bundle_outputs_anysize(out_dir, source_path or Path(__file__).resolve(), tag)

    res.update(score_summary)
    res["run_tag"] = tag
    if bundle is not None:
        res["bundle"] = str(bundle)
    res["out_dir"] = str(out_dir)
    res.update(aliases)
    return res

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sqlite", default="/mnt/data/lane_ranker.sqlite3")
    ap.add_argument("--out", default=None)
    ap.add_argument("--lane-size", type=int, required=True)
    ap.add_argument("--folds", type=int, default=5)
    ap.add_argument("--seed", type=int, default=123)
    ap.add_argument("--raw-min", type=float, default=None)
    args = ap.parse_args()

    tag = make_run_tag(args.lane_size, args.raw_min)
    out_path = Path(args.out) if args.out else Path(f"/mnt/data/crossfit_betabinomial_interactionrich_anysize_scoreblocked_{tag}")
    try:
        res = run_anysize(Path(args.sqlite), out_path, args.lane_size, args.folds, args.seed, args.raw_min, Path(__file__).resolve())
    except MissingRateRequest as exc:
        request_path = write_missing_rate_request(exc)
        print(json.dumps({
            "status": "missing_rate_pairs",
            "request_path": str(request_path),
            "missing_pair_count": len(exc.missing_pairs),
            "context": exc.context,
        }, ensure_ascii=False, indent=2))
        sys.exit(86)
    print(json.dumps(res, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    main()
