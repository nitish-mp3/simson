function t(t,e,i,s){var a,o=arguments.length,n=o<3?e:null===s?s=Object.getOwnPropertyDescriptor(e,i):s;if("object"==typeof Reflect&&"function"==typeof Reflect.decorate)n=Reflect.decorate(t,e,i,s);else for(var r=t.length-1;r>=0;r--)(a=t[r])&&(n=(o<3?a(n):o>3?a(e,i,n):a(e,i))||n);return o>3&&n&&Object.defineProperty(e,i,n),n}"function"==typeof SuppressedError&&SuppressedError;const e=globalThis,i=e.ShadowRoot&&(void 0===e.ShadyCSS||e.ShadyCSS.nativeShadow)&&"adoptedStyleSheets"in Document.prototype&&"replace"in CSSStyleSheet.prototype,s=Symbol(),a=new WeakMap;let o=class{constructor(t,e,i){if(this._$cssResult$=!0,i!==s)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=t,this.t=e}get styleSheet(){let t=this.o;const e=this.t;if(i&&void 0===t){const i=void 0!==e&&1===e.length;i&&(t=a.get(e)),void 0===t&&((this.o=t=new CSSStyleSheet).replaceSync(this.cssText),i&&a.set(e,t))}return t}toString(){return this.cssText}};const n=(t,...e)=>{const i=1===t.length?t[0]:e.reduce((e,i,s)=>e+(t=>{if(!0===t._$cssResult$)return t.cssText;if("number"==typeof t)return t;throw Error("Value passed to 'css' function must be a 'css' function result: "+t+". Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.")})(i)+t[s+1],t[0]);return new o(i,t,s)},r=i?t=>t:t=>t instanceof CSSStyleSheet?(t=>{let e="";for(const i of t.cssRules)e+=i.cssText;return(t=>new o("string"==typeof t?t:t+"",void 0,s))(e)})(t):t,{is:l,defineProperty:c,getOwnPropertyDescriptor:d,getOwnPropertyNames:h,getOwnPropertySymbols:p,getPrototypeOf:u}=Object,g=globalThis,_=g.trustedTypes,v=_?_.emptyScript:"",b=g.reactiveElementPolyfillSupport,m=(t,e)=>t,f={toAttribute(t,e){switch(e){case Boolean:t=t?v:null;break;case Object:case Array:t=null==t?t:JSON.stringify(t)}return t},fromAttribute(t,e){let i=t;switch(e){case Boolean:i=null!==t;break;case Number:i=null===t?null:Number(t);break;case Object:case Array:try{i=JSON.parse(t)}catch(t){i=null}}return i}},y=(t,e)=>!l(t,e),w={attribute:!0,type:String,converter:f,reflect:!1,useDefault:!1,hasChanged:y};Symbol.metadata??=Symbol("metadata"),g.litPropertyMetadata??=new WeakMap;let x=class extends HTMLElement{static addInitializer(t){this._$Ei(),(this.l??=[]).push(t)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(t,e=w){if(e.state&&(e.attribute=!1),this._$Ei(),this.prototype.hasOwnProperty(t)&&((e=Object.create(e)).wrapped=!0),this.elementProperties.set(t,e),!e.noAccessor){const i=Symbol(),s=this.getPropertyDescriptor(t,i,e);void 0!==s&&c(this.prototype,t,s)}}static getPropertyDescriptor(t,e,i){const{get:s,set:a}=d(this.prototype,t)??{get(){return this[e]},set(t){this[e]=t}};return{get:s,set(e){const o=s?.call(this);a?.call(this,e),this.requestUpdate(t,o,i)},configurable:!0,enumerable:!0}}static getPropertyOptions(t){return this.elementProperties.get(t)??w}static _$Ei(){if(this.hasOwnProperty(m("elementProperties")))return;const t=u(this);t.finalize(),void 0!==t.l&&(this.l=[...t.l]),this.elementProperties=new Map(t.elementProperties)}static finalize(){if(this.hasOwnProperty(m("finalized")))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(m("properties"))){const t=this.properties,e=[...h(t),...p(t)];for(const i of e)this.createProperty(i,t[i])}const t=this[Symbol.metadata];if(null!==t){const e=litPropertyMetadata.get(t);if(void 0!==e)for(const[t,i]of e)this.elementProperties.set(t,i)}this._$Eh=new Map;for(const[t,e]of this.elementProperties){const i=this._$Eu(t,e);void 0!==i&&this._$Eh.set(i,t)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(t){const e=[];if(Array.isArray(t)){const i=new Set(t.flat(1/0).reverse());for(const t of i)e.unshift(r(t))}else void 0!==t&&e.push(r(t));return e}static _$Eu(t,e){const i=e.attribute;return!1===i?void 0:"string"==typeof i?i:"string"==typeof t?t.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){this._$ES=new Promise(t=>this.enableUpdating=t),this._$AL=new Map,this._$E_(),this.requestUpdate(),this.constructor.l?.forEach(t=>t(this))}addController(t){(this._$EO??=new Set).add(t),void 0!==this.renderRoot&&this.isConnected&&t.hostConnected?.()}removeController(t){this._$EO?.delete(t)}_$E_(){const t=new Map,e=this.constructor.elementProperties;for(const i of e.keys())this.hasOwnProperty(i)&&(t.set(i,this[i]),delete this[i]);t.size>0&&(this._$Ep=t)}createRenderRoot(){const t=this.shadowRoot??this.attachShadow(this.constructor.shadowRootOptions);return((t,s)=>{if(i)t.adoptedStyleSheets=s.map(t=>t instanceof CSSStyleSheet?t:t.styleSheet);else for(const i of s){const s=document.createElement("style"),a=e.litNonce;void 0!==a&&s.setAttribute("nonce",a),s.textContent=i.cssText,t.appendChild(s)}})(t,this.constructor.elementStyles),t}connectedCallback(){this.renderRoot??=this.createRenderRoot(),this.enableUpdating(!0),this._$EO?.forEach(t=>t.hostConnected?.())}enableUpdating(t){}disconnectedCallback(){this._$EO?.forEach(t=>t.hostDisconnected?.())}attributeChangedCallback(t,e,i){this._$AK(t,i)}_$ET(t,e){const i=this.constructor.elementProperties.get(t),s=this.constructor._$Eu(t,i);if(void 0!==s&&!0===i.reflect){const a=(void 0!==i.converter?.toAttribute?i.converter:f).toAttribute(e,i.type);this._$Em=t,null==a?this.removeAttribute(s):this.setAttribute(s,a),this._$Em=null}}_$AK(t,e){const i=this.constructor,s=i._$Eh.get(t);if(void 0!==s&&this._$Em!==s){const t=i.getPropertyOptions(s),a="function"==typeof t.converter?{fromAttribute:t.converter}:void 0!==t.converter?.fromAttribute?t.converter:f;this._$Em=s;const o=a.fromAttribute(e,t.type);this[s]=o??this._$Ej?.get(s)??o,this._$Em=null}}requestUpdate(t,e,i,s=!1,a){if(void 0!==t){const o=this.constructor;if(!1===s&&(a=this[t]),i??=o.getPropertyOptions(t),!((i.hasChanged??y)(a,e)||i.useDefault&&i.reflect&&a===this._$Ej?.get(t)&&!this.hasAttribute(o._$Eu(t,i))))return;this.C(t,e,i)}!1===this.isUpdatePending&&(this._$ES=this._$EP())}C(t,e,{useDefault:i,reflect:s,wrapped:a},o){i&&!(this._$Ej??=new Map).has(t)&&(this._$Ej.set(t,o??e??this[t]),!0!==a||void 0!==o)||(this._$AL.has(t)||(this.hasUpdated||i||(e=void 0),this._$AL.set(t,e)),!0===s&&this._$Em!==t&&(this._$Eq??=new Set).add(t))}async _$EP(){this.isUpdatePending=!0;try{await this._$ES}catch(t){Promise.reject(t)}const t=this.scheduleUpdate();return null!=t&&await t,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){if(!this.isUpdatePending)return;if(!this.hasUpdated){if(this.renderRoot??=this.createRenderRoot(),this._$Ep){for(const[t,e]of this._$Ep)this[t]=e;this._$Ep=void 0}const t=this.constructor.elementProperties;if(t.size>0)for(const[e,i]of t){const{wrapped:t}=i,s=this[e];!0!==t||this._$AL.has(e)||void 0===s||this.C(e,void 0,i,s)}}let t=!1;const e=this._$AL;try{t=this.shouldUpdate(e),t?(this.willUpdate(e),this._$EO?.forEach(t=>t.hostUpdate?.()),this.update(e)):this._$EM()}catch(e){throw t=!1,this._$EM(),e}t&&this._$AE(e)}willUpdate(t){}_$AE(t){this._$EO?.forEach(t=>t.hostUpdated?.()),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(t)),this.updated(t)}_$EM(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(t){return!0}update(t){this._$Eq&&=this._$Eq.forEach(t=>this._$ET(t,this[t])),this._$EM()}updated(t){}firstUpdated(t){}};x.elementStyles=[],x.shadowRootOptions={mode:"open"},x[m("elementProperties")]=new Map,x[m("finalized")]=new Map,b?.({ReactiveElement:x}),(g.reactiveElementVersions??=[]).push("2.1.2");const $=globalThis,C=t=>t,k=$.trustedTypes,S=k?k.createPolicy("lit-html",{createHTML:t=>t}):void 0,A="$lit$",M=`lit$${Math.random().toFixed(9).slice(2)}$`,L="?"+M,T=`<${L}>`,E=document,R=()=>E.createComment(""),H=t=>null===t||"object"!=typeof t&&"function"!=typeof t,z=Array.isArray,V="[ \t\n\f\r]",I=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,D=/-->/g,P=/>/g,O=RegExp(`>|${V}(?:([^\\s"'>=/]+)(${V}*=${V}*(?:[^ \t\n\f\r"'\`<>=]|("|')|))|$)`,"g"),N=/'/g,U=/"/g,B=/^(?:script|style|textarea|title)$/i,W=(t,...e)=>({_$litType$:1,strings:t,values:e}),j=Symbol.for("lit-noChange"),q=Symbol.for("lit-nothing"),Z=new WeakMap,K=E.createTreeWalker(E,129);function Y(t,e){if(!z(t)||!t.hasOwnProperty("raw"))throw Error("invalid template strings array");return void 0!==S?S.createHTML(e):e}const Q=(t,e)=>{const i=t.length-1,s=[];let a,o=2===e?"<svg>":3===e?"<math>":"",n=I;for(let e=0;e<i;e++){const i=t[e];let r,l,c=-1,d=0;for(;d<i.length&&(n.lastIndex=d,l=n.exec(i),null!==l);)d=n.lastIndex,n===I?"!--"===l[1]?n=D:void 0!==l[1]?n=P:void 0!==l[2]?(B.test(l[2])&&(a=RegExp("</"+l[2],"g")),n=O):void 0!==l[3]&&(n=O):n===O?">"===l[0]?(n=a??I,c=-1):void 0===l[1]?c=-2:(c=n.lastIndex-l[2].length,r=l[1],n=void 0===l[3]?O:'"'===l[3]?U:N):n===U||n===N?n=O:n===D||n===P?n=I:(n=O,a=void 0);const h=n===O&&t[e+1].startsWith("/>")?" ":"";o+=n===I?i+T:c>=0?(s.push(r),i.slice(0,c)+A+i.slice(c)+M+h):i+M+(-2===c?e:h)}return[Y(t,o+(t[i]||"<?>")+(2===e?"</svg>":3===e?"</math>":"")),s]};class F{constructor({strings:t,_$litType$:e},i){let s;this.parts=[];let a=0,o=0;const n=t.length-1,r=this.parts,[l,c]=Q(t,e);if(this.el=F.createElement(l,i),K.currentNode=this.el.content,2===e||3===e){const t=this.el.content.firstChild;t.replaceWith(...t.childNodes)}for(;null!==(s=K.nextNode())&&r.length<n;){if(1===s.nodeType){if(s.hasAttributes())for(const t of s.getAttributeNames())if(t.endsWith(A)){const e=c[o++],i=s.getAttribute(t).split(M),n=/([.?@])?(.*)/.exec(e);r.push({type:1,index:a,name:n[2],strings:i,ctor:"."===n[1]?et:"?"===n[1]?it:"@"===n[1]?st:tt}),s.removeAttribute(t)}else t.startsWith(M)&&(r.push({type:6,index:a}),s.removeAttribute(t));if(B.test(s.tagName)){const t=s.textContent.split(M),e=t.length-1;if(e>0){s.textContent=k?k.emptyScript:"";for(let i=0;i<e;i++)s.append(t[i],R()),K.nextNode(),r.push({type:2,index:++a});s.append(t[e],R())}}}else if(8===s.nodeType)if(s.data===L)r.push({type:2,index:a});else{let t=-1;for(;-1!==(t=s.data.indexOf(M,t+1));)r.push({type:7,index:a}),t+=M.length-1}a++}}static createElement(t,e){const i=E.createElement("template");return i.innerHTML=t,i}}function J(t,e,i=t,s){if(e===j)return e;let a=void 0!==s?i._$Co?.[s]:i._$Cl;const o=H(e)?void 0:e._$litDirective$;return a?.constructor!==o&&(a?._$AO?.(!1),void 0===o?a=void 0:(a=new o(t),a._$AT(t,i,s)),void 0!==s?(i._$Co??=[])[s]=a:i._$Cl=a),void 0!==a&&(e=J(t,a._$AS(t,e.values),a,s)),e}class G{constructor(t,e){this._$AV=[],this._$AN=void 0,this._$AD=t,this._$AM=e}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(t){const{el:{content:e},parts:i}=this._$AD,s=(t?.creationScope??E).importNode(e,!0);K.currentNode=s;let a=K.nextNode(),o=0,n=0,r=i[0];for(;void 0!==r;){if(o===r.index){let e;2===r.type?e=new X(a,a.nextSibling,this,t):1===r.type?e=new r.ctor(a,r.name,r.strings,this,t):6===r.type&&(e=new at(a,this,t)),this._$AV.push(e),r=i[++n]}o!==r?.index&&(a=K.nextNode(),o++)}return K.currentNode=E,s}p(t){let e=0;for(const i of this._$AV)void 0!==i&&(void 0!==i.strings?(i._$AI(t,i,e),e+=i.strings.length-2):i._$AI(t[e])),e++}}class X{get _$AU(){return this._$AM?._$AU??this._$Cv}constructor(t,e,i,s){this.type=2,this._$AH=q,this._$AN=void 0,this._$AA=t,this._$AB=e,this._$AM=i,this.options=s,this._$Cv=s?.isConnected??!0}get parentNode(){let t=this._$AA.parentNode;const e=this._$AM;return void 0!==e&&11===t?.nodeType&&(t=e.parentNode),t}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(t,e=this){t=J(this,t,e),H(t)?t===q||null==t||""===t?(this._$AH!==q&&this._$AR(),this._$AH=q):t!==this._$AH&&t!==j&&this._(t):void 0!==t._$litType$?this.$(t):void 0!==t.nodeType?this.T(t):(t=>z(t)||"function"==typeof t?.[Symbol.iterator])(t)?this.k(t):this._(t)}O(t){return this._$AA.parentNode.insertBefore(t,this._$AB)}T(t){this._$AH!==t&&(this._$AR(),this._$AH=this.O(t))}_(t){this._$AH!==q&&H(this._$AH)?this._$AA.nextSibling.data=t:this.T(E.createTextNode(t)),this._$AH=t}$(t){const{values:e,_$litType$:i}=t,s="number"==typeof i?this._$AC(t):(void 0===i.el&&(i.el=F.createElement(Y(i.h,i.h[0]),this.options)),i);if(this._$AH?._$AD===s)this._$AH.p(e);else{const t=new G(s,this),i=t.u(this.options);t.p(e),this.T(i),this._$AH=t}}_$AC(t){let e=Z.get(t.strings);return void 0===e&&Z.set(t.strings,e=new F(t)),e}k(t){z(this._$AH)||(this._$AH=[],this._$AR());const e=this._$AH;let i,s=0;for(const a of t)s===e.length?e.push(i=new X(this.O(R()),this.O(R()),this,this.options)):i=e[s],i._$AI(a),s++;s<e.length&&(this._$AR(i&&i._$AB.nextSibling,s),e.length=s)}_$AR(t=this._$AA.nextSibling,e){for(this._$AP?.(!1,!0,e);t!==this._$AB;){const e=C(t).nextSibling;C(t).remove(),t=e}}setConnected(t){void 0===this._$AM&&(this._$Cv=t,this._$AP?.(t))}}class tt{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(t,e,i,s,a){this.type=1,this._$AH=q,this._$AN=void 0,this.element=t,this.name=e,this._$AM=s,this.options=a,i.length>2||""!==i[0]||""!==i[1]?(this._$AH=Array(i.length-1).fill(new String),this.strings=i):this._$AH=q}_$AI(t,e=this,i,s){const a=this.strings;let o=!1;if(void 0===a)t=J(this,t,e,0),o=!H(t)||t!==this._$AH&&t!==j,o&&(this._$AH=t);else{const s=t;let n,r;for(t=a[0],n=0;n<a.length-1;n++)r=J(this,s[i+n],e,n),r===j&&(r=this._$AH[n]),o||=!H(r)||r!==this._$AH[n],r===q?t=q:t!==q&&(t+=(r??"")+a[n+1]),this._$AH[n]=r}o&&!s&&this.j(t)}j(t){t===q?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,t??"")}}class et extends tt{constructor(){super(...arguments),this.type=3}j(t){this.element[this.name]=t===q?void 0:t}}class it extends tt{constructor(){super(...arguments),this.type=4}j(t){this.element.toggleAttribute(this.name,!!t&&t!==q)}}class st extends tt{constructor(t,e,i,s,a){super(t,e,i,s,a),this.type=5}_$AI(t,e=this){if((t=J(this,t,e,0)??q)===j)return;const i=this._$AH,s=t===q&&i!==q||t.capture!==i.capture||t.once!==i.once||t.passive!==i.passive,a=t!==q&&(i===q||s);s&&this.element.removeEventListener(this.name,this,i),a&&this.element.addEventListener(this.name,this,t),this._$AH=t}handleEvent(t){"function"==typeof this._$AH?this._$AH.call(this.options?.host??this.element,t):this._$AH.handleEvent(t)}}class at{constructor(t,e,i){this.element=t,this.type=6,this._$AN=void 0,this._$AM=e,this.options=i}get _$AU(){return this._$AM._$AU}_$AI(t){J(this,t)}}const ot=$.litHtmlPolyfillSupport;ot?.(F,X),($.litHtmlVersions??=[]).push("3.3.2");const nt=globalThis;class rt extends x{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){const t=super.createRenderRoot();return this.renderOptions.renderBefore??=t.firstChild,t}update(t){const e=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(t),this._$Do=((t,e,i)=>{const s=i?.renderBefore??e;let a=s._$litPart$;if(void 0===a){const t=i?.renderBefore??null;s._$litPart$=a=new X(e.insertBefore(R(),t),t,void 0,i??{})}return a._$AI(t),a})(e,this.renderRoot,this.renderOptions)}connectedCallback(){super.connectedCallback(),this._$Do?.setConnected(!0)}disconnectedCallback(){super.disconnectedCallback(),this._$Do?.setConnected(!1)}render(){return j}}rt._$litElement$=!0,rt.finalized=!0,nt.litElementHydrateSupport?.({LitElement:rt});const lt=nt.litElementPolyfillSupport;lt?.({LitElement:rt}),(nt.litElementVersions??=[]).push("4.2.2");const ct=t=>(e,i)=>{void 0!==i?i.addInitializer(()=>{customElements.define(t,e)}):customElements.define(t,e)},dt={attribute:!0,type:String,converter:f,reflect:!1,hasChanged:y},ht=(t=dt,e,i)=>{const{kind:s,metadata:a}=i;let o=globalThis.litPropertyMetadata.get(a);if(void 0===o&&globalThis.litPropertyMetadata.set(a,o=new Map),"setter"===s&&((t=Object.create(t)).wrapped=!0),o.set(i.name,t),"accessor"===s){const{name:s}=i;return{set(i){const a=e.get.call(this);e.set.call(this,i),this.requestUpdate(s,a,t,!0,i)},init(e){return void 0!==e&&this.C(s,void 0,t,e),e}}}if("setter"===s){const{name:s}=i;return function(i){const a=this[s];e.call(this,i),this.requestUpdate(s,a,t,!0,i)}}throw Error("Unsupported decorator location: "+s)};function pt(t){return(e,i)=>"object"==typeof i?ht(t,e,i):((t,e,i)=>{const s=e.hasOwnProperty(i);return e.constructor.createProperty(i,t),s?Object.getOwnPropertyDescriptor(e,i):void 0})(t,e,i)}function ut(t){return pt({...t,state:!0,attribute:!1})}function gt(t,e){return(e,i,s)=>((t,e,i)=>(i.configurable=!0,i.enumerable=!0,Reflect.decorate&&"object"!=typeof e&&Object.defineProperty(t,e,i),i))(e,i,{get(){return(e=>e.renderRoot?.querySelector(t)??null)(this)}})}const _t=n`
  :host {
    --voip-primary: var(--primary-color, #03a9f4);
    --voip-primary-text: var(--primary-text-color, #212121);
    --voip-secondary-text: var(--secondary-text-color, #727272);
    --voip-disabled: var(--disabled-text-color, #bdbdbd);
    --voip-divider: var(--divider-color, rgba(0, 0, 0, 0.12));
    --voip-card-bg: var(--card-background-color, #fff);
    --voip-surface: var(--ha-card-background, var(--voip-card-bg));
    --voip-error: var(--error-color, #db4437);
    --voip-success: var(--success-color, #43a047);
    --voip-warning: var(--warning-color, #ffa726);
    --voip-info: var(--info-color, #039be5);
    --voip-radius: var(--ha-card-border-radius, 12px);
    --voip-shadow: var(
      --ha-card-box-shadow,
      0 2px 2px 0 rgba(0, 0, 0, 0.14),
      0 1px 5px 0 rgba(0, 0, 0, 0.12),
      0 3px 1px -2px rgba(0, 0, 0, 0.2)
    );
    --voip-btn-size: 56px;
    --voip-btn-size-sm: 44px;

    display: block;
    font-family: var(--paper-font-body1_-_font-family, "Roboto", sans-serif);
    color: var(--voip-primary-text);
  }

  *,
  *::before,
  *::after {
    box-sizing: border-box;
  }

  ha-card {
    overflow: hidden;
    border-radius: var(--voip-radius);
  }
`,vt=n`
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 50%;
    cursor: pointer;
    transition: background-color 0.2s ease, transform 0.1s ease,
      box-shadow 0.2s ease;
    user-select: none;
    -webkit-tap-highlight-color: transparent;
    outline: none;
    font-size: 0;
    padding: 0;
  }

  .btn:focus-visible {
    box-shadow: 0 0 0 3px var(--voip-primary);
  }

  .btn:active {
    transform: scale(0.93);
  }

  .btn--lg {
    width: var(--voip-btn-size);
    height: var(--voip-btn-size);
  }

  .btn--md {
    width: var(--voip-btn-size-sm);
    height: var(--voip-btn-size-sm);
  }

  .btn--sm {
    width: 36px;
    height: 36px;
  }

  .btn--call {
    background-color: var(--voip-success);
    color: #fff;
  }

  .btn--call:hover {
    background-color: #388e3c;
  }

  .btn--hangup {
    background-color: var(--voip-error);
    color: #fff;
  }

  .btn--hangup:hover {
    background-color: #c62828;
  }

  .btn--action {
    background-color: var(--voip-surface);
    color: var(--voip-primary-text);
    border: 1px solid var(--voip-divider);
  }

  .btn--action:hover {
    background-color: var(--voip-divider);
  }

  .btn--action.active {
    background-color: var(--voip-primary);
    color: #fff;
    border-color: var(--voip-primary);
  }

  .btn--icon {
    background: none;
    color: var(--voip-secondary-text);
    border: none;
  }

  .btn--icon:hover {
    color: var(--voip-primary-text);
    background-color: rgba(0, 0, 0, 0.06);
  }
`,bt=n`
  .dialpad-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    padding: 16px;
    max-width: 280px;
    margin: 0 auto;
  }

  .dialpad-key {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 64px;
    height: 64px;
    margin: 0 auto;
    border: none;
    border-radius: 50%;
    background-color: var(--voip-surface);
    border: 1px solid var(--voip-divider);
    cursor: pointer;
    transition: background-color 0.15s ease, transform 0.1s ease;
    user-select: none;
    -webkit-tap-highlight-color: transparent;
    font-family: inherit;
    outline: none;
  }

  .dialpad-key:focus-visible {
    box-shadow: 0 0 0 3px var(--voip-primary);
  }

  .dialpad-key:hover {
    background-color: var(--voip-divider);
  }

  .dialpad-key:active {
    transform: scale(0.92);
    background-color: var(--voip-primary);
    color: #fff;
  }

  .dialpad-key__digit {
    font-size: 24px;
    font-weight: 500;
    line-height: 1;
    color: var(--voip-primary-text);
  }

  .dialpad-key__letters {
    font-size: 9px;
    letter-spacing: 2px;
    text-transform: uppercase;
    color: var(--voip-secondary-text);
    margin-top: 2px;
  }

  .dialpad-key:active .dialpad-key__digit,
  .dialpad-key:active .dialpad-key__letters {
    color: #fff;
  }
`,mt=n`
  .call-controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 16px;
  }

  .call-controls__label {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    color: var(--voip-secondary-text);
  }

  .call-controls__label .btn--action.active + span {
    color: var(--voip-primary);
  }
`,ft=n`
  .status-dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    margin-right: 6px;
    flex-shrink: 0;
  }

  .status-dot--available {
    background-color: var(--voip-success);
  }

  .status-dot--busy,
  .status-dot--ringing {
    background-color: var(--voip-warning);
  }

  .status-dot--offline {
    background-color: var(--voip-disabled);
  }

  .status-dot--dnd {
    background-color: var(--voip-error);
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: 12px;
    font-size: 12px;
    font-weight: 500;
    line-height: 1.5;
  }

  .badge--idle {
    background-color: rgba(0, 0, 0, 0.06);
    color: var(--voip-secondary-text);
  }

  .badge--ringing {
    background-color: rgba(255, 167, 38, 0.15);
    color: #e65100;
    animation: pulse 1.5s infinite;
  }

  .badge--connected {
    background-color: rgba(67, 160, 71, 0.15);
    color: #2e7d32;
  }

  .badge--on_hold {
    background-color: rgba(3, 155, 229, 0.15);
    color: #01579b;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
`,yt=n`
  .history-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .history-item {
    display: flex;
    align-items: center;
    padding: 10px 16px;
    border-bottom: 1px solid var(--voip-divider);
    gap: 12px;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .history-item:last-child {
    border-bottom: none;
  }

  .history-item:hover {
    background-color: rgba(0, 0, 0, 0.04);
  }

  .history-item__icon {
    flex-shrink: 0;
    width: 20px;
    text-align: center;
  }

  .history-item__icon--inbound {
    color: var(--voip-success);
  }

  .history-item__icon--outbound {
    color: var(--voip-primary);
  }

  .history-item__icon--missed {
    color: var(--voip-error);
  }

  .history-item__info {
    flex: 1;
    min-width: 0;
  }

  .history-item__name {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .history-item__number {
    font-size: 12px;
    color: var(--voip-secondary-text);
  }

  .history-item__meta {
    text-align: right;
    flex-shrink: 0;
  }

  .history-item__time {
    font-size: 12px;
    color: var(--voip-secondary-text);
  }

  .history-item__duration {
    font-size: 11px;
    color: var(--voip-disabled);
  }
`,wt=n`
  .popup-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background-color: rgba(0, 0, 0, 0.6);
    animation: fadeIn 0.2s ease;
  }

  .popup-card {
    background-color: var(--voip-surface);
    border-radius: var(--voip-radius);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    max-width: 400px;
    width: 90vw;
    max-height: 90vh;
    overflow-y: auto;
    animation: slideUp 0.25s ease;
  }

  .popup-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--voip-divider);
  }

  .popup-body {
    padding: 20px;
  }

  .popup-footer {
    display: flex;
    justify-content: center;
    gap: 16px;
    padding: 16px 20px;
    border-top: 1px solid var(--voip-divider);
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(30px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
`,xt=n`
  .wizard {
    padding: 20px;
  }

  .wizard-progress {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    margin-bottom: 24px;
  }

  .wizard-progress__step {
    width: 32px;
    height: 4px;
    border-radius: 2px;
    background-color: var(--voip-divider);
    transition: background-color 0.3s ease;
  }

  .wizard-progress__step--active {
    background-color: var(--voip-primary);
  }

  .wizard-progress__step--completed {
    background-color: var(--voip-success);
  }

  .wizard-title {
    font-size: 20px;
    font-weight: 500;
    margin: 0 0 8px;
    color: var(--voip-primary-text);
  }

  .wizard-subtitle {
    font-size: 14px;
    color: var(--voip-secondary-text);
    margin: 0 0 20px;
    line-height: 1.5;
  }

  .wizard-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid var(--voip-divider);
  }

  .wizard-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 20px;
    border: none;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.2s, transform 0.1s;
    font-family: inherit;
    outline: none;
  }

  .wizard-btn:focus-visible {
    box-shadow: 0 0 0 3px var(--voip-primary);
  }

  .wizard-btn:active {
    transform: scale(0.97);
  }

  .wizard-btn--primary {
    background-color: var(--voip-primary);
    color: #fff;
  }

  .wizard-btn--primary:hover {
    filter: brightness(1.1);
  }

  .wizard-btn--secondary {
    background: none;
    color: var(--voip-secondary-text);
  }

  .wizard-btn--secondary:hover {
    background-color: rgba(0, 0, 0, 0.06);
  }

  .wizard-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
`,$t=n`
  .diag-table {
    width: 100%;
    border-collapse: collapse;
  }

  .diag-row {
    display: flex;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--voip-divider);
    gap: 12px;
  }

  .diag-row:last-child {
    border-bottom: none;
  }

  .diag-icon {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
  }

  .diag-icon--pass {
    background-color: rgba(67, 160, 71, 0.15);
    color: var(--voip-success);
  }

  .diag-icon--fail {
    background-color: rgba(219, 68, 55, 0.15);
    color: var(--voip-error);
  }

  .diag-icon--warning {
    background-color: rgba(255, 167, 38, 0.15);
    color: var(--voip-warning);
  }

  .diag-icon--running {
    background-color: rgba(3, 155, 229, 0.15);
    color: var(--voip-info);
    animation: spin 1s linear infinite;
  }

  .diag-icon--pending {
    background-color: rgba(0, 0, 0, 0.06);
    color: var(--voip-disabled);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .diag-info {
    flex: 1;
    min-width: 0;
  }

  .diag-name {
    font-size: 14px;
    font-weight: 500;
  }

  .diag-message {
    font-size: 12px;
    color: var(--voip-secondary-text);
    margin-top: 2px;
  }

  .diag-time {
    font-size: 12px;
    color: var(--voip-disabled);
    flex-shrink: 0;
  }
`,Ct=n`
  @media (max-width: 600px) {
    :host {
      --voip-btn-size: 48px;
      --voip-btn-size-sm: 40px;
    }

    .dialpad-key {
      width: 56px;
      height: 56px;
    }

    .dialpad-key__digit {
      font-size: 20px;
    }

    .dialpad-grid {
      gap: 8px;
      padding: 12px;
    }

    .call-controls {
      gap: 10px;
      padding: 12px;
    }

    .popup-card {
      max-width: 100%;
      width: 100vw;
      max-height: 100vh;
      border-radius: 0;
    }
  }

  @media (max-width: 380px) {
    .dialpad-key {
      width: 48px;
      height: 48px;
    }

    .dialpad-key__digit {
      font-size: 18px;
    }

    .dialpad-key__letters {
      display: none;
    }
  }
`,kt=n`
  .form-group {
    margin-bottom: 16px;
  }

  .form-label {
    display: block;
    font-size: 13px;
    font-weight: 500;
    color: var(--voip-secondary-text);
    margin-bottom: 6px;
  }

  .form-input {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--voip-divider);
    border-radius: 8px;
    font-size: 14px;
    font-family: inherit;
    color: var(--voip-primary-text);
    background-color: var(--voip-surface);
    outline: none;
    transition: border-color 0.2s;
  }

  .form-input:focus {
    border-color: var(--voip-primary);
    box-shadow: 0 0 0 2px rgba(3, 169, 244, 0.2);
  }

  .form-input::placeholder {
    color: var(--voip-disabled);
  }

  .form-select {
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--voip-divider);
    border-radius: 8px;
    font-size: 14px;
    font-family: inherit;
    color: var(--voip-primary-text);
    background-color: var(--voip-surface);
    outline: none;
    cursor: pointer;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath fill='%23727272' d='M6 8L1 3h10z'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 12px center;
    padding-right: 32px;
  }

  .form-select:focus {
    border-color: var(--voip-primary);
    box-shadow: 0 0 0 2px rgba(3, 169, 244, 0.2);
  }

  .form-radio-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .form-radio {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--voip-divider);
    border-radius: 8px;
    cursor: pointer;
    transition: border-color 0.2s, background-color 0.2s;
  }

  .form-radio:hover {
    background-color: rgba(0, 0, 0, 0.02);
  }

  .form-radio--selected {
    border-color: var(--voip-primary);
    background-color: rgba(3, 169, 244, 0.06);
  }

  .form-radio input[type="radio"] {
    margin-top: 2px;
    accent-color: var(--voip-primary);
  }

  .form-radio__label {
    font-size: 14px;
    font-weight: 500;
  }

  .form-radio__description {
    font-size: 12px;
    color: var(--voip-secondary-text);
    margin-top: 2px;
    line-height: 1.4;
  }
`,St={"card.title":"VoIP Phone","card.no_config":"No configuration provided","card.loading":"Loading...","call.idle":"Idle","call.ringing":"Ringing","call.dialing":"Dialing","call.connected":"Connected","call.on_hold":"On Hold","call.transferring":"Transferring","call.ended":"Call Ended","call.incoming":"Incoming Call","call.duration":"Duration","call.unknown_caller":"Unknown Caller","dialpad.title":"Dialpad","dialpad.placeholder":"Enter number...","dialpad.call":"Call","dialpad.hangup":"Hang Up","dialpad.backspace":"Backspace","controls.mute":"Mute","controls.unmute":"Unmute","controls.hold":"Hold","controls.unhold":"Resume","controls.hangup":"Hang Up","controls.transfer":"Transfer","controls.record":"Record","controls.stop_record":"Stop Recording","controls.speaker":"Speaker","controls.keypad":"Keypad","controls.accept":"Accept","controls.reject":"Reject","controls.audio_device":"Audio Device","ext.title":"Extensions","ext.available":"Available","ext.busy":"Busy","ext.ringing":"Ringing","ext.offline":"Offline","ext.dnd":"Do Not Disturb","quickdial.title":"Quick Dial","history.title":"Recent Calls","history.no_calls":"No recent calls","history.inbound":"Inbound","history.outbound":"Outbound","history.missed":"Missed","history.today":"Today","history.yesterday":"Yesterday","popup.incoming_call":"Incoming Call","popup.active_call":"Active Call","popup.camera_snapshot":"Doorbell Camera","onboarding.title":"VoIP Setup","onboarding.skip":"Skip (use defaults)","onboarding.back":"Back","onboarding.next":"Next","onboarding.finish":"Finish Setup","onboarding.condensed":"Quick Setup","onboarding.full":"Full Setup","onboarding.step1.title":"Welcome to HA VoIP","onboarding.step1.subtitle":"Let's set up voice calling for your smart home. First, we need microphone permission.","onboarding.step1.request_mic":"Grant Microphone Access","onboarding.step1.mic_granted":"Microphone access granted","onboarding.step1.mic_denied":"Microphone access denied. Please allow it in your browser settings.","onboarding.step2.title":"Network Test","onboarding.step2.subtitle":"Checking your network for VoIP compatibility.","onboarding.step2.running":"Running tests...","onboarding.step2.complete":"Network tests complete","onboarding.step3.title":"Mode Selection","onboarding.step3.subtitle":"Choose how VoIP will operate.","onboarding.step3.local":"Local Only","onboarding.step3.local_desc":"Calls stay within your local network. Best for intercom and room-to-room calling.","onboarding.step3.federated":"Federated","onboarding.step3.federated_desc":"Connect to external SIP providers for PSTN calls. Requires port forwarding or a SIP trunk.","onboarding.step4.title":"Extension Assignment","onboarding.step4.subtitle":"Map Home Assistant users to extension numbers for internal calling.","onboarding.step4.user":"User","onboarding.step4.extension":"Extension","onboarding.step4.add":"Add Extension","onboarding.step5.title":"Certificate Setup","onboarding.step5.subtitle":"WebRTC requires secure connections. Choose a certificate option.","onboarding.step5.auto":"Automatic (Let's Encrypt)","onboarding.step5.auto_desc":"Automatically obtain and renew certificates via Let's Encrypt.","onboarding.step5.manual":"Manual","onboarding.step5.manual_desc":"Provide your own certificate and key files.","onboarding.step5.self_signed":"Self-Signed","onboarding.step5.self_signed_desc":"Generate a self-signed certificate. Not recommended for production.","onboarding.step6.title":"Test Call","onboarding.step6.subtitle":"Make a loopback test call to verify everything works.","onboarding.step6.start":"Start Test Call","onboarding.step6.testing":"Testing...","onboarding.step6.success":"Test call succeeded! Everything is working.","onboarding.step6.failure":"Test call failed. Check diagnostics for details.","diag.title":"Network Diagnostics","diag.run_all":"Run All Tests","diag.export":"Export as JSON","diag.wss":"WebSocket (WSS)","diag.stun":"STUN Server","diag.turn":"TURN Server","diag.rtt":"Network RTT","diag.one_way_audio":"One-Way Audio Test","diag.ice_candidates":"ICE Candidates","diag.pass":"Pass","diag.fail":"Fail","diag.warning":"Warning","diag.pending":"Pending","diag.running":"Running","config.title":"VoIP Card Configuration","config.card_title":"Card Title","config.entity":"VoIP Entity","config.show_recent":"Show Recent Calls","config.recent_count":"Number of Recent Calls","config.show_dialpad":"Show Dialpad","config.show_diagnostics":"Show Diagnostics Button","config.compact_mode":"Compact Mode","config.enable_dtmf":"Enable DTMF Tones","config.auto_answer":"Auto-Answer Calls","config.ringtone":"Ringtone URL","config.quick_dial":"Quick Dial Entries","config.add_quick_dial":"Add Quick Dial","config.name":"Name","config.number":"Number","config.icon":"Icon","config.remove":"Remove"};function At(t,e,...i){let s;if(e?.localize){const i=e.localize(t);i&&i!==t&&(s=i)}return s||(s=St[t]),s?(i.length>0&&i.forEach((t,e)=>{s=s.replace(`{${e}}`,String(t))}),s):t}function Mt(t){const e=Math.floor(t/3600),i=Math.floor(t%3600/60),s=Math.floor(t%60),a=String(i).padStart(2,"0"),o=String(s).padStart(2,"0");return e>0?`${e}:${a}:${o}`:`${a}:${o}`}function Lt(t,e,i,s){const a=new CustomEvent(e,{bubbles:!0,composed:!0,cancelable:!1,detail:i??{}});t.dispatchEvent(a)}const Tt={1:[697,1209],2:[697,1336],3:[697,1477],4:[770,1209],5:[770,1336],6:[770,1477],7:[852,1209],8:[852,1336],9:[852,1477],"*":[941,1209],0:[941,1336],"#":[941,1477]},Et={1:"",2:"ABC",3:"DEF",4:"GHI",5:"JKL",6:"MNO",7:"PQRS",8:"TUV",9:"WXYZ","*":"",0:"+","#":""},Rt=["1","2","3","4","5","6","7","8","9","*","0","#"];let Ht=class extends rt{constructor(){super(...arguments),this.callState="idle",this.enableDtmf=!0,this._number="",this._audioCtx=null,this._handleKeyboard=t=>{const e=t.key;Tt[e]?(t.preventDefault(),this._pressKey(e)):"Backspace"===e?this._handleBackspace():"Enter"===e&&this._number?"idle"===this.callState&&this._handleCall():"Escape"===e&&this._handleHangup()}}connectedCallback(){super.connectedCallback(),this.addEventListener("keydown",this._handleKeyboard)}disconnectedCallback(){super.disconnectedCallback(),this.removeEventListener("keydown",this._handleKeyboard),this._audioCtx&&(this._audioCtx.close(),this._audioCtx=null)}render(){const t="connected"===this.callState||"dialing"===this.callState||"on_hold"===this.callState;return W`
      <!-- Number display -->
      <div class="dialpad-display" role="textbox" aria-label="${At("dialpad.placeholder",this.hass)}">
        <input
          id="dial-input"
          class="dialpad-display__input"
          type="tel"
          .value=${this._number}
          placeholder=${At("dialpad.placeholder",this.hass)}
          @input=${this._handleInput}
          aria-label="${At("dialpad.placeholder",this.hass)}"
        />
        ${this._number?W`
              <button
                class="btn btn--icon btn--sm dialpad-display__backspace"
                @click=${this._handleBackspace}
                aria-label=${At("dialpad.backspace",this.hass)}
              >
                <svg viewBox="0 0 24 24" width="20" height="20">
                  <path fill="currentColor" d="M22,3H7C6.31,3 5.77,3.35 5.41,3.88L0,12L5.41,20.11C5.77,20.64 6.31,21 7,21H22A2,2 0 0,0 24,19V5A2,2 0 0,0 22,3M19,15.59L17.59,17L14,13.41L10.41,17L9,15.59L12.59,12L9,8.41L10.41,7L14,10.59L17.59,7L19,8.41L15.41,12" />
                </svg>
              </button>
            `:q}
      </div>

      <!-- Key grid -->
      <div class="dialpad-grid" role="group" aria-label="${At("dialpad.title",this.hass)}">
        ${Rt.map(t=>W`
            <button
              class="dialpad-key"
              data-key=${t}
              @click=${()=>this._pressKey(t)}
              @touchstart=${e=>{e.preventDefault(),this._pressKey(t)}}
              aria-label="${t} ${Et[t]||""}"
            >
              <span class="dialpad-key__digit">${t}</span>
              ${Et[t]?W`<span class="dialpad-key__letters">${Et[t]}</span>`:q}
            </button>
          `)}
      </div>

      <!-- Action buttons -->
      <div class="dialpad-actions">
        ${t?W`
              <button
                class="btn btn--lg btn--hangup"
                @click=${this._handleHangup}
                aria-label=${At("controls.hangup",this.hass)}
              >
                <svg viewBox="0 0 24 24" width="28" height="28">
                  <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
                </svg>
              </button>
            `:W`
              <button
                class="btn btn--lg btn--call"
                @click=${this._handleCall}
                ?disabled=${!this._number}
                aria-label=${At("dialpad.call",this.hass)}
              >
                <svg viewBox="0 0 24 24" width="28" height="28">
                  <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
                </svg>
              </button>
            `}
      </div>
    `}_pressKey(t){this._number+=t,this.enableDtmf&&this._playDtmf(t),"connected"===this.callState&&Lt(this,"voip-dtmf",{digit:t})}_handleBackspace(){this._number=this._number.slice(0,-1)}_handleInput(t){const e=t.target;this._number=e.value.replace(/[^0-9*#]/g,"")}_handleCall(){this._number&&Lt(this,"voip-call",{number:this._number})}_handleHangup(){Lt(this,"voip-hangup")}_playDtmf(t){const e=Tt[t];if(e)try{this._audioCtx||(this._audioCtx=new AudioContext);const t=.15,i=this._audioCtx.currentTime,s=this._audioCtx.createOscillator(),a=this._audioCtx.createOscillator(),o=this._audioCtx.createGain();s.frequency.value=e[0],a.frequency.value=e[1],s.type="sine",a.type="sine",o.gain.setValueAtTime(.15,i),o.gain.exponentialRampToValueAtTime(.001,i+t),s.connect(o),a.connect(o),o.connect(this._audioCtx.destination),s.start(i),a.start(i),s.stop(i+t),a.stop(i+t)}catch{}}clear(){this._number=""}get number(){return this._number}set number(t){this._number=t.replace(/[^0-9*#]/g,"")}static get additionalStyles(){return[]}createRenderRoot(){return super.createRenderRoot()}updated(t){super.updated(t),this._inputEl&&this._inputEl.value!==this._number&&(this._inputEl.value=this._number)}};Ht.styles=[_t,vt,bt,Ct],t([pt({attribute:!1})],Ht.prototype,"hass",void 0),t([pt({type:String,reflect:!0})],Ht.prototype,"callState",void 0),t([pt({type:Boolean})],Ht.prototype,"enableDtmf",void 0),t([ut()],Ht.prototype,"_number",void 0),t([gt("#dial-input")],Ht.prototype,"_inputEl",void 0),Ht=t([ct("ha-voip-dialpad")],Ht);const zt=document.createElement("style");zt.textContent="\n  ha-voip-dialpad .dialpad-display {\n    display: flex;\n    align-items: center;\n    padding: 8px 16px;\n    gap: 8px;\n  }\n\n  ha-voip-dialpad .dialpad-display__input {\n    flex: 1;\n    border: none;\n    outline: none;\n    font-size: 24px;\n    font-weight: 500;\n    text-align: center;\n    background: transparent;\n    color: var(--voip-primary-text, #212121);\n    font-family: inherit;\n    letter-spacing: 2px;\n  }\n\n  ha-voip-dialpad .dialpad-display__input::placeholder {\n    font-size: 14px;\n    letter-spacing: normal;\n    color: var(--voip-disabled, #bdbdbd);\n  }\n\n  ha-voip-dialpad .dialpad-actions {\n    display: flex;\n    justify-content: center;\n    padding: 12px 16px 16px;\n  }\n",document.querySelector("style[data-voip-dialpad]")||(zt.setAttribute("data-voip-dialpad",""),document.head.appendChild(zt));let Vt=class extends rt{constructor(){super(...arguments),this._elapsed=0,this._showKeypad=!1,this._showDeviceMenu=!1,this._audioDevices=[],this._dragOffsetY=0,this._isDragging=!1,this._cameraUrl=null,this._timerInterval=null,this._touchStartY=0}connectedCallback(){super.connectedCallback(),this._loadAudioDevices()}disconnectedCallback(){super.disconnectedCallback(),this._stopTimer()}updated(t){super.updated(t),t.has("callState")&&this._onCallStateChanged(),t.has("cameraEntityId")&&this.cameraEntityId&&this._loadCameraSnapshot()}render(){if(!this.callState)return q;const t="ringing"===this.callState.state&&"inbound"===this.callState.direction,e="connected"===this.callState.state||"on_hold"===this.callState.state;return W`
      <div
        class="popup-overlay"
        @click=${this._handleOverlayClick}
        role="dialog"
        aria-label=${At(t?"popup.incoming_call":"popup.active_call",this.hass)}
      >
        <div
          class="popup-card"
          style=${this._isDragging?`transform: translateY(${this._dragOffsetY}px)`:""}
          @click=${t=>t.stopPropagation()}
          @touchstart=${this._handleTouchStart}
          @touchmove=${this._handleTouchMove}
          @touchend=${this._handleTouchEnd}
        >
          <!-- Drag handle (mobile) -->
          <div class="drag-handle"></div>

          <!-- Caller info -->
          ${this._renderCallerInfo()}

          <!-- Camera snapshot (doorbell integration) -->
          ${this._renderCameraSnapshot()}

          <!-- Call timer (active calls) -->
          ${e?this._renderTimer():q}

          <!-- Incoming call actions -->
          ${t?this._renderIncomingActions():q}

          <!-- Active call controls -->
          ${e?this._renderActiveControls():q}

          <!-- Inline keypad -->
          ${this._showKeypad&&e?this._renderInlineKeypad():q}
        </div>
      </div>
    `}_renderCallerInfo(){const t=this.callState,e=t.remoteName||At("call.unknown_caller",this.hass),i=e.split(" ").map(t=>t[0]).join("").slice(0,2).toUpperCase();let s="",a="";switch(t.state){case"ringing":s="call-status--ringing",a="inbound"===t.direction?At("call.incoming",this.hass):At("call.ringing",this.hass);break;case"dialing":s="call-status--ringing",a=At("call.dialing",this.hass);break;case"connected":s="call-status--connected",a=At("call.connected",this.hass);break;case"on_hold":s="call-status--on_hold",a=At("call.on_hold",this.hass);break;default:a=At(`call.${t.state}`,this.hass)}return W`
      <div class="caller-info">
        <div class="caller-avatar" aria-hidden="true">${i}</div>
        <p class="caller-name">${e}</p>
        <p class="caller-number">${t.remoteNumber}</p>
        <p class="call-status ${s}">${a}</p>
      </div>
    `}_renderCameraSnapshot(){return this._cameraUrl?W`
      <div class="camera-snapshot">
        <img
          src=${this._cameraUrl}
          alt=${At("popup.camera_snapshot",this.hass)}
          loading="lazy"
        />
      </div>
    `:q}_renderTimer(){return W`
      <div class="call-timer" role="timer" aria-live="polite">
        ${Mt(this._elapsed)}
      </div>
    `}_renderIncomingActions(){return W`
      <div class="incoming-actions">
        <div class="incoming-action-label incoming-action-label--reject">
          <button
            class="btn btn--lg btn--hangup"
            @click=${this._handleReject}
            aria-label=${At("controls.reject",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="28" height="28">
              <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
            </svg>
          </button>
          <span>${At("controls.reject",this.hass)}</span>
        </div>
        <div class="incoming-action-label incoming-action-label--accept">
          <button
            class="btn btn--lg btn--call"
            @click=${this._handleAccept}
            aria-label=${At("controls.accept",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="28" height="28">
              <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
            </svg>
          </button>
          <span>${At("controls.accept",this.hass)}</span>
        </div>
      </div>
    `}_renderActiveControls(){const t=this.callState;return W`
      <div class="call-controls">
        <!-- Mute -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${t.isMuted?"active":""}"
            @click=${this._handleMute}
            aria-label=${t.isMuted?At("controls.unmute",this.hass):At("controls.mute",this.hass)}
            aria-pressed=${t.isMuted}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              ${t.isMuted?W`<path fill="currentColor" d="M19,11C19,12.19 18.66,13.3 18.1,14.28L16.87,13.05C17.14,12.43 17.3,11.74 17.3,11H19M15,11.16L9,5.18V5A3,3 0 0,1 12,2A3,3 0 0,1 15,5V11L15,11.16M4.27,3L3,4.27L9.01,10.28V11A3,3 0 0,0 12.01,14C12.22,14 12.42,13.97 12.62,13.92L14.01,15.31C13.39,15.6 12.72,15.78 12.01,15.83V19H14.01V21H10.01V19H12.01V15.83C9.24,15.56 7.01,13.5 7.01,11H8.71C8.71,13 10.41,14.29 12.01,14.29C12.33,14.29 12.63,14.24 12.92,14.15L11.51,12.74C11.35,12.77 11.18,12.8 11.01,12.8A1.8,1.8 0 0,1 9.21,11V10.28L4.27,3Z" />`:W`<path fill="currentColor" d="M12,2A3,3 0 0,1 15,5V11A3,3 0 0,1 12,14A3,3 0 0,1 9,11V5A3,3 0 0,1 12,2M19,11C19,14.53 16.39,17.44 13,17.93V21H11V17.93C7.61,17.44 5,14.53 5,11H7A5,5 0 0,0 12,16A5,5 0 0,0 17,11H19Z" />`}
            </svg>
          </button>
          <span>${t.isMuted?At("controls.unmute",this.hass):At("controls.mute",this.hass)}</span>
        </div>

        <!-- Speaker -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${t.isSpeaker?"active":""}"
            @click=${this._handleSpeaker}
            aria-label=${At("controls.speaker",this.hass)}
            aria-pressed=${t.isSpeaker}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M14,3.23V5.29C16.89,6.15 19,8.83 19,12C19,15.17 16.89,17.84 14,18.7V20.77C18,19.86 21,16.28 21,12C21,7.72 18,4.14 14,3.23M16.5,12C16.5,10.23 15.5,8.71 14,7.97V16C15.5,15.29 16.5,13.76 16.5,12M3,9V15H7L12,20V4L7,9H3Z" />
            </svg>
          </button>
          <span>${At("controls.speaker",this.hass)}</span>
        </div>

        <!-- Hold -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${t.isOnHold?"active":""}"
            @click=${this._handleHold}
            aria-label=${t.isOnHold?At("controls.unhold",this.hass):At("controls.hold",this.hass)}
            aria-pressed=${t.isOnHold}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M14,19H18V5H14M6,19H10V5H6V19Z" />
            </svg>
          </button>
          <span>${t.isOnHold?At("controls.unhold",this.hass):At("controls.hold",this.hass)}</span>
        </div>

        <!-- Record -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${t.isRecording?"active":""}"
            @click=${this._handleRecord}
            aria-label=${t.isRecording?At("controls.stop_record",this.hass):At("controls.record",this.hass)}
            aria-pressed=${t.isRecording}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              ${t.isRecording?W`<path fill="currentColor" d="M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M9,8H15V16H9V8Z" />`:W`<path fill="currentColor" d="M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M12,7A5,5 0 0,0 7,12A5,5 0 0,0 12,17A5,5 0 0,0 17,12A5,5 0 0,0 12,7Z" />`}
            </svg>
          </button>
          <span>${t.isRecording?At("controls.stop_record",this.hass):At("controls.record",this.hass)}</span>
        </div>

        <!-- Keypad toggle -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${this._showKeypad?"active":""}"
            @click=${this._toggleKeypad}
            aria-label=${At("controls.keypad",this.hass)}
            aria-pressed=${this._showKeypad}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M12,19A2,2 0 0,0 14,17A2,2 0 0,0 12,15A2,2 0 0,0 10,17A2,2 0 0,0 12,19M6,1H18A2,2 0 0,1 20,3V21A2,2 0 0,1 18,23H6A2,2 0 0,1 4,21V3A2,2 0 0,1 6,1M6,3V21H18V3H6M8,5H10V7H8V5M12,5H14V7H12V5M16,5H18V7H16V5M8,9H10V11H8V9M12,9H14V11H12V9M16,9H18V11H16V9M8,13H10V15H8V13M12,13H14V15H12V13M16,13H18V15H16V13Z" />
            </svg>
          </button>
          <span>${At("controls.keypad",this.hass)}</span>
        </div>

        <!-- Transfer -->
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action"
            @click=${this._handleTransfer}
            aria-label=${At("controls.transfer",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path fill="currentColor" d="M18,13V5H20V13H18M14,5V13H16V5H14M11,5L6,10L11,15V12C15.39,12 19.17,13.58 22,16.28C20.63,11.11 16.33,7.15 11,6.34V5Z" />
            </svg>
          </button>
          <span>${At("controls.transfer",this.hass)}</span>
        </div>
      </div>

      <!-- Audio device selector (relative container) -->
      <div style="position:relative; text-align:center; padding-bottom:8px;">
        <button
          class="btn btn--sm btn--icon"
          @click=${this._toggleDeviceMenu}
          aria-label=${At("controls.audio_device",this.hass)}
        >
          <svg viewBox="0 0 24 24" width="18" height="18">
            <path fill="currentColor" d="M12,1C7,1 3,5 3,10V17A3,3 0 0,0 6,20H9V12H5V10A7,7 0 0,1 12,3A7,7 0 0,1 19,10V12H15V20H18A3,3 0 0,0 21,17V10C21,5 16.97,1 12,1Z" />
          </svg>
        </button>
        ${this._showDeviceMenu?this._renderDeviceMenu():q}
      </div>

      <!-- Hangup button -->
      <div class="popup-footer">
        <button
          class="btn btn--lg btn--hangup"
          @click=${this._handleHangup}
          aria-label=${At("controls.hangup",this.hass)}
        >
          <svg viewBox="0 0 24 24" width="28" height="28">
            <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
          </svg>
        </button>
      </div>
    `}_renderInlineKeypad(){return W`
      <div class="keypad-inline">
        <ha-voip-dialpad
          .hass=${this.hass}
          callState=${this.callState.state}
          @voip-dtmf=${this._handleDtmf}
        ></ha-voip-dialpad>
      </div>
    `}_renderDeviceMenu(){return W`
      <div class="device-menu" role="listbox" aria-label=${At("controls.audio_device",this.hass)}>
        ${0===this._audioDevices.length?W`<div class="device-menu__item">${At("card.loading",this.hass)}</div>`:this._audioDevices.map(t=>W`
                <button
                  class="device-menu__item"
                  role="option"
                  @click=${()=>this._selectDevice(t)}
                >
                  ${"audioinput"===t.kind?"🎤 ":"🔊 "}${t.label}
                </button>
              `)}
      </div>
    `}_handleAccept(){Lt(this,"voip-answer",{call_id:this.callState?.id})}_handleReject(){Lt(this,"voip-hangup",{call_id:this.callState?.id})}_handleHangup(){Lt(this,"voip-hangup",{call_id:this.callState?.id})}_handleMute(){Lt(this,"voip-mute",{call_id:this.callState?.id,mute:!this.callState?.isMuted})}_handleSpeaker(){Lt(this,"voip-speaker",{call_id:this.callState?.id,speaker:!this.callState?.isSpeaker})}_handleHold(){Lt(this,"voip-hold",{call_id:this.callState?.id,hold:!this.callState?.isOnHold})}_handleRecord(){Lt(this,"voip-record",{call_id:this.callState?.id,record:!this.callState?.isRecording})}_handleTransfer(){Lt(this,"voip-transfer-start",{call_id:this.callState?.id})}_handleDtmf(t){Lt(this,"voip-dtmf",{call_id:this.callState?.id,digit:t.detail.digit})}_toggleKeypad(){this._showKeypad=!this._showKeypad}_toggleDeviceMenu(){this._showDeviceMenu=!this._showDeviceMenu,this._showDeviceMenu&&this._loadAudioDevices()}_selectDevice(t){Lt(this,"voip-device-change",{deviceId:t.deviceId,kind:t.kind}),this._showDeviceMenu=!1}_handleOverlayClick(){"ringing"!==this.callState?.state&&Lt(this,"voip-popup-minimize")}_handleTouchStart(t){this._touchStartY=t.touches[0].clientY,this._isDragging=!0,this._dragOffsetY=0}_handleTouchMove(t){if(!this._isDragging)return;const e=t.touches[0].clientY-this._touchStartY;e>0&&(this._dragOffsetY=e)}_handleTouchEnd(){this._isDragging&&(this._isDragging=!1,this._dragOffsetY>150&&Lt(this,"voip-popup-minimize"),this._dragOffsetY=0)}_onCallStateChanged(){this.callState?"connected"===this.callState.state&&!this._timerInterval&&this.callState.connectTime?this._startTimer():"connected"!==this.callState.state&&"on_hold"!==this.callState.state&&this._stopTimer():this._stopTimer()}_startTimer(){this._updateElapsed(),this._timerInterval=setInterval(()=>this._updateElapsed(),1e3)}_stopTimer(){this._timerInterval&&(clearInterval(this._timerInterval),this._timerInterval=null)}_updateElapsed(){this.callState?.connectTime&&(this._elapsed=Math.floor((Date.now()-this.callState.connectTime)/1e3))}async _loadAudioDevices(){try{const t=await navigator.mediaDevices.enumerateDevices();this._audioDevices=t.filter(t=>"audioinput"===t.kind||"audiooutput"===t.kind).map(t=>({deviceId:t.deviceId,label:t.label||`${t.kind} (${t.deviceId.slice(0,6)})`,kind:t.kind}))}catch{this._audioDevices=[]}}async _loadCameraSnapshot(){if(this.cameraEntityId&&this.hass)try{const t=await this.hass.callWS({type:"camera_thumbnail",entity_id:this.cameraEntityId});t?.content&&(this._cameraUrl=`data:${t.content_type};base64,${t.content}`)}catch{this._cameraUrl=`/api/camera_proxy/${this.cameraEntityId}`}}};Vt.styles=[_t,vt,mt,wt,Ct,n`
      .caller-info {
        text-align: center;
        padding: 24px 20px 8px;
      }

      .caller-avatar {
        width: 72px;
        height: 72px;
        border-radius: 50%;
        background-color: var(--voip-primary);
        color: #fff;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 28px;
        font-weight: 500;
        margin: 0 auto 12px;
      }

      .caller-name {
        font-size: 22px;
        font-weight: 500;
        margin: 0 0 4px;
      }

      .caller-number {
        font-size: 14px;
        color: var(--voip-secondary-text);
        margin: 0 0 8px;
      }

      .call-status {
        font-size: 13px;
        font-weight: 500;
        margin: 0;
      }

      .call-status--ringing {
        color: var(--voip-warning);
        animation: pulse 1.5s infinite;
      }

      .call-status--connected {
        color: var(--voip-success);
      }

      .call-status--on_hold {
        color: var(--voip-info);
      }

      .call-timer {
        font-size: 32px;
        font-weight: 300;
        text-align: center;
        padding: 12px 0;
        font-variant-numeric: tabular-nums;
        letter-spacing: 2px;
      }

      .incoming-actions {
        display: flex;
        justify-content: center;
        gap: 48px;
        padding: 24px 20px 32px;
      }

      .incoming-action-label {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 8px;
        font-size: 13px;
        font-weight: 500;
      }

      .incoming-action-label--accept {
        color: var(--voip-success);
      }

      .incoming-action-label--reject {
        color: var(--voip-error);
      }

      .camera-snapshot {
        margin: 8px 16px;
        border-radius: 8px;
        overflow: hidden;
        background-color: #000;
        aspect-ratio: 16 / 9;
      }

      .camera-snapshot img {
        width: 100%;
        height: 100%;
        object-fit: contain;
        display: block;
      }

      .device-menu {
        position: absolute;
        bottom: 100%;
        left: 50%;
        transform: translateX(-50%);
        background-color: var(--voip-surface);
        border-radius: 8px;
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
        min-width: 220px;
        max-height: 200px;
        overflow-y: auto;
        z-index: 10;
      }

      .device-menu__item {
        display: block;
        width: 100%;
        padding: 10px 16px;
        border: none;
        background: none;
        text-align: left;
        font-size: 13px;
        font-family: inherit;
        color: var(--voip-primary-text);
        cursor: pointer;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .device-menu__item:hover {
        background-color: rgba(0, 0, 0, 0.06);
      }

      .device-menu__item--active {
        color: var(--voip-primary);
        font-weight: 500;
      }

      .keypad-inline {
        padding: 0 16px 8px;
      }

      .drag-handle {
        width: 40px;
        height: 4px;
        border-radius: 2px;
        background-color: var(--voip-divider);
        margin: 8px auto 0;
      }

      @media (max-width: 600px) {
        .popup-card {
          position: fixed;
          bottom: 0;
          left: 0;
          right: 0;
          max-width: 100%;
          width: 100%;
          border-radius: 16px 16px 0 0;
          max-height: 95vh;
          animation: slideUpMobile 0.3s ease;
        }

        @keyframes slideUpMobile {
          from {
            transform: translateY(100%);
          }
          to {
            transform: translateY(0);
          }
        }

        .incoming-actions {
          padding-bottom: calc(32px + env(safe-area-inset-bottom, 0px));
        }
      }
    `],t([pt({attribute:!1})],Vt.prototype,"hass",void 0),t([pt({attribute:!1})],Vt.prototype,"callState",void 0),t([pt({type:String})],Vt.prototype,"cameraEntityId",void 0),t([ut()],Vt.prototype,"_elapsed",void 0),t([ut()],Vt.prototype,"_showKeypad",void 0),t([ut()],Vt.prototype,"_showDeviceMenu",void 0),t([ut()],Vt.prototype,"_audioDevices",void 0),t([ut()],Vt.prototype,"_dragOffsetY",void 0),t([ut()],Vt.prototype,"_isDragging",void 0),t([ut()],Vt.prototype,"_cameraUrl",void 0),t([gt(".popup-card")],Vt.prototype,"_card",void 0),Vt=t([ct("ha-voip-call-popup")],Vt);const It=["welcome","network_test","mode_selection","extension_assignment","certificate_setup","test_call"];let Dt=class extends rt{constructor(){super(...arguments),this._currentStep="welcome",this._condensedMode=!1,this._micPermission="prompt",this._networkTests=[],this._networkTestRunning=!1,this._selectedMode="local",this._extensions=[],this._certMode="auto",this._certPath="",this._testCallState="idle",this._webrtc=null}disconnectedCallback(){super.disconnectedCallback(),this._webrtc&&(this._webrtc.hangup(),this._webrtc=null)}render(){const t=It.indexOf(this._currentStep),e=It.length;return W`
      <div class="wizard">
        <!-- Progress indicator -->
        ${this._condensedMode?q:W`
              <div class="wizard-progress" role="progressbar" aria-valuenow=${t+1} aria-valuemax=${e}>
                ${It.map((e,i)=>{let s="wizard-progress__step";return i<t?s+=" wizard-progress__step--completed":i===t&&(s+=" wizard-progress__step--active"),W`<div class=${s}></div>`})}
              </div>
            `}

        <!-- Step content -->
        ${this._renderStep()}

        <!-- Navigation -->
        ${this._renderNav()}
      </div>
    `}_renderStep(){if(this._condensedMode)return this._renderCondensedStep();switch(this._currentStep){case"welcome":return this._renderWelcome();case"network_test":return this._renderNetworkTest();case"mode_selection":return this._renderModeSelection();case"extension_assignment":return this._renderExtensionAssignment();case"certificate_setup":return this._renderCertificateSetup();case"test_call":return this._renderTestCall();default:return q}}_renderWelcome(){return W`
      <h2 class="wizard-title">${At("onboarding.step1.title",this.hass)}</h2>
      <p class="wizard-subtitle">${At("onboarding.step1.subtitle",this.hass)}</p>

      <!-- Condensed vs full choice -->
      <div class="condensed-choice">
        <button class="wizard-btn wizard-btn--primary" @click=${()=>{this._condensedMode=!1}}>
          ${At("onboarding.full",this.hass)}
        </button>
        <button class="wizard-btn wizard-btn--secondary" @click=${this._startCondensed}>
          ${At("onboarding.condensed",this.hass)}
        </button>
      </div>

      <!-- Microphone permission -->
      <div class="form-group">
        ${"granted"===this._micPermission?W`
              <div style="display:flex;align-items:center;gap:8px;color:var(--voip-success)">
                <svg viewBox="0 0 24 24" width="24" height="24">
                  <path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                </svg>
                <span>${At("onboarding.step1.mic_granted",this.hass)}</span>
              </div>
            `:"denied"===this._micPermission?W`
                <div style="display:flex;align-items:center;gap:8px;color:var(--voip-error)">
                  <svg viewBox="0 0 24 24" width="24" height="24">
                    <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
                  </svg>
                  <span>${At("onboarding.step1.mic_denied",this.hass)}</span>
                </div>
              `:W`
                <button class="wizard-btn wizard-btn--primary" @click=${this._requestMicrophone}>
                  <svg viewBox="0 0 24 24" width="18" height="18" style="margin-right:6px">
                    <path fill="currentColor" d="M12,2A3,3 0 0,1 15,5V11A3,3 0 0,1 12,14A3,3 0 0,1 9,11V5A3,3 0 0,1 12,2M19,11C19,14.53 16.39,17.44 13,17.93V21H11V17.93C7.61,17.44 5,14.53 5,11H7A5,5 0 0,0 12,16A5,5 0 0,0 17,11H19Z" />
                  </svg>
                  ${At("onboarding.step1.request_mic",this.hass)}
                </button>
              `}
      </div>
    `}_renderNetworkTest(){return W`
      <h2 class="wizard-title">${At("onboarding.step2.title",this.hass)}</h2>
      <p class="wizard-subtitle">${At("onboarding.step2.subtitle",this.hass)}</p>

      <div class="diag-table">
        ${this._networkTests.map(t=>W`
            <div class="diag-row">
              <div class="diag-icon diag-icon--${t.status}">
                ${this._renderStatusIcon(t.status)}
              </div>
              <div class="diag-info">
                <div class="diag-name">${t.name}</div>
                ${t.message?W`<div class="diag-message">${t.message}</div>`:q}
              </div>
              ${null!=t.durationMs?W`<div class="diag-time">${t.durationMs}ms</div>`:q}
            </div>
          `)}
      </div>

      ${this._networkTestRunning||0!==this._networkTests.length?q:W`
            <button class="wizard-btn wizard-btn--primary" @click=${this._runNetworkTests}>
              ${At("diag.run_all",this.hass)}
            </button>
          `}
      ${this._networkTestRunning?W`<p style="text-align:center;color:var(--voip-secondary-text)">${At("onboarding.step2.running",this.hass)}</p>`:q}
    `}_renderModeSelection(){return W`
      <h2 class="wizard-title">${At("onboarding.step3.title",this.hass)}</h2>
      <p class="wizard-subtitle">${At("onboarding.step3.subtitle",this.hass)}</p>

      <div class="form-radio-group">
        <label class="form-radio ${"local"===this._selectedMode?"form-radio--selected":""}">
          <input
            type="radio"
            name="mode"
            value="local"
            .checked=${"local"===this._selectedMode}
            @change=${()=>{this._selectedMode="local"}}
          />
          <div>
            <div class="form-radio__label">${At("onboarding.step3.local",this.hass)}</div>
            <div class="form-radio__description">${At("onboarding.step3.local_desc",this.hass)}</div>
          </div>
        </label>
        <label class="form-radio ${"federated"===this._selectedMode?"form-radio--selected":""}">
          <input
            type="radio"
            name="mode"
            value="federated"
            .checked=${"federated"===this._selectedMode}
            @change=${()=>{this._selectedMode="federated"}}
          />
          <div>
            <div class="form-radio__label">${At("onboarding.step3.federated",this.hass)}</div>
            <div class="form-radio__description">${At("onboarding.step3.federated_desc",this.hass)}</div>
          </div>
        </label>
      </div>
    `}_renderExtensionAssignment(){return W`
      <h2 class="wizard-title">${At("onboarding.step4.title",this.hass)}</h2>
      <p class="wizard-subtitle">${At("onboarding.step4.subtitle",this.hass)}</p>

      ${this._extensions.map((t,e)=>W`
          <div class="ext-row">
            <input
              class="form-input"
              type="text"
              placeholder=${At("onboarding.step4.user",this.hass)}
              .value=${t.name}
              @input=${t=>this._updateExtName(e,t.target.value)}
            />
            <input
              class="form-input ext-number"
              type="tel"
              placeholder="100"
              .value=${t.number}
              @input=${t=>this._updateExtNumber(e,t.target.value)}
            />
            <button
              class="btn btn--sm btn--icon"
              @click=${()=>this._removeExtension(e)}
              aria-label=${At("config.remove",this.hass)}
            >
              <svg viewBox="0 0 24 24" width="18" height="18">
                <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
              </svg>
            </button>
          </div>
        `)}

      <button class="wizard-btn wizard-btn--secondary" @click=${this._addExtension}>
        + ${At("onboarding.step4.add",this.hass)}
      </button>
    `}_renderCertificateSetup(){return W`
      <h2 class="wizard-title">${At("onboarding.step5.title",this.hass)}</h2>
      <p class="wizard-subtitle">${At("onboarding.step5.subtitle",this.hass)}</p>

      <div class="form-radio-group">
        <label class="form-radio ${"auto"===this._certMode?"form-radio--selected":""}">
          <input
            type="radio"
            name="cert"
            value="auto"
            .checked=${"auto"===this._certMode}
            @change=${()=>{this._certMode="auto"}}
          />
          <div>
            <div class="form-radio__label">${At("onboarding.step5.auto",this.hass)}</div>
            <div class="form-radio__description">${At("onboarding.step5.auto_desc",this.hass)}</div>
          </div>
        </label>

        <label class="form-radio ${"manual"===this._certMode?"form-radio--selected":""}">
          <input
            type="radio"
            name="cert"
            value="manual"
            .checked=${"manual"===this._certMode}
            @change=${()=>{this._certMode="manual"}}
          />
          <div>
            <div class="form-radio__label">${At("onboarding.step5.manual",this.hass)}</div>
            <div class="form-radio__description">${At("onboarding.step5.manual_desc",this.hass)}</div>
          </div>
        </label>

        <label class="form-radio ${"self_signed"===this._certMode?"form-radio--selected":""}">
          <input
            type="radio"
            name="cert"
            value="self_signed"
            .checked=${"self_signed"===this._certMode}
            @change=${()=>{this._certMode="self_signed"}}
          />
          <div>
            <div class="form-radio__label">${At("onboarding.step5.self_signed",this.hass)}</div>
            <div class="form-radio__description">${At("onboarding.step5.self_signed_desc",this.hass)}</div>
          </div>
        </label>
      </div>

      ${"manual"===this._certMode?W`
            <div class="form-group" style="margin-top:12px">
              <label class="form-label">Certificate path</label>
              <input
                class="form-input"
                type="text"
                placeholder="/ssl/fullchain.pem"
                .value=${this._certPath}
                @input=${t=>{this._certPath=t.target.value}}
              />
            </div>
          `:q}
    `}_renderTestCall(){return W`
      <h2 class="wizard-title">${At("onboarding.step6.title",this.hass)}</h2>
      <p class="wizard-subtitle">${At("onboarding.step6.subtitle",this.hass)}</p>

      ${"idle"===this._testCallState?W`
            <div style="text-align:center">
              <button class="wizard-btn wizard-btn--primary" @click=${this._startTestCall}>
                ${At("onboarding.step6.start",this.hass)}
              </button>
            </div>
          `:q}

      ${"testing"===this._testCallState?W`
            <div class="test-call-result">
              <div class="diag-icon diag-icon--running" style="width:48px;height:48px;margin:0 auto 12px;font-size:24px;">
                <svg viewBox="0 0 24 24" width="24" height="24">
                  <path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z" />
                </svg>
              </div>
              <p class="test-call-result__message">${At("onboarding.step6.testing",this.hass)}</p>
            </div>
          `:q}

      ${"success"===this._testCallState?W`
            <div class="test-call-result test-call-result--success">
              <div class="test-call-result__icon">
                <svg viewBox="0 0 24 24" width="48" height="48">
                  <path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                </svg>
              </div>
              <p class="test-call-result__message">${At("onboarding.step6.success",this.hass)}</p>
            </div>
          `:q}

      ${"failure"===this._testCallState?W`
            <div class="test-call-result test-call-result--failure">
              <div class="test-call-result__icon">
                <svg viewBox="0 0 24 24" width="48" height="48">
                  <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
                </svg>
              </div>
              <p class="test-call-result__message">${At("onboarding.step6.failure",this.hass)}</p>
            </div>
          `:q}
    `}_renderCondensedStep(){return"welcome"===this._currentStep?W`
        <h2 class="wizard-title">${At("onboarding.condensed",this.hass)}</h2>
        <p class="wizard-subtitle">
          The following defaults will be applied:
        </p>
        <ul style="font-size:14px;line-height:2;color:var(--voip-secondary-text)">
          <li>Mode: <strong>Local</strong></li>
          <li>Certificate: <strong>Automatic (Let's Encrypt)</strong></li>
          <li>Extensions: <strong>Auto-assigned from HA users</strong></li>
        </ul>

        <div class="form-group">
          ${"granted"!==this._micPermission?W`
                <button class="wizard-btn wizard-btn--primary" @click=${this._requestMicrophone}>
                  ${At("onboarding.step1.request_mic",this.hass)}
                </button>
              `:W`
                <div style="display:flex;align-items:center;gap:8px;color:var(--voip-success)">
                  <svg viewBox="0 0 24 24" width="20" height="20">
                    <path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z" />
                  </svg>
                  <span>${At("onboarding.step1.mic_granted",this.hass)}</span>
                </div>
              `}
        </div>
      `:this._renderTestCall()}_renderNav(){const t=It.indexOf(this._currentStep),e=t===It.length-1,i=0===t;return this._condensedMode?"welcome"===this._currentStep?W`
          <div class="wizard-actions">
            <button class="wizard-btn wizard-btn--secondary" @click=${()=>{this._condensedMode=!1}}>
              ${At("onboarding.full",this.hass)}
            </button>
            <button
              class="wizard-btn wizard-btn--primary"
              ?disabled=${"granted"!==this._micPermission}
              @click=${()=>{this._currentStep="test_call"}}
            >
              ${At("onboarding.next",this.hass)}
            </button>
          </div>
        `:W`
        <div class="wizard-actions">
          <button class="wizard-btn wizard-btn--secondary" @click=${()=>{this._currentStep="welcome"}}>
            ${At("onboarding.back",this.hass)}
          </button>
          <button class="wizard-btn wizard-btn--primary" @click=${this._finishSetup}>
            ${At("onboarding.finish",this.hass)}
          </button>
        </div>
      `:W`
      <div class="wizard-actions">
        <div>
          ${i?W`
                <button class="wizard-btn wizard-btn--secondary" @click=${this._skipAll}>
                  ${At("onboarding.skip",this.hass)}
                </button>
              `:W`
                <button class="wizard-btn wizard-btn--secondary" @click=${this._prevStep}>
                  ${At("onboarding.back",this.hass)}
                </button>
              `}
        </div>
        <div>
          ${e?W`
                <button class="wizard-btn wizard-btn--primary" @click=${this._finishSetup}>
                  ${At("onboarding.finish",this.hass)}
                </button>
              `:W`
                <button class="wizard-btn wizard-btn--primary" @click=${this._nextStep}>
                  ${At("onboarding.next",this.hass)}
                </button>
              `}
        </div>
      </div>
    `}_nextStep(){const t=It.indexOf(this._currentStep);t<It.length-1&&(this._currentStep=It[t+1])}_prevStep(){const t=It.indexOf(this._currentStep);t>0&&(this._currentStep=It[t-1])}_skipAll(){this._finishSetup()}_startCondensed(){this._condensedMode=!0,this._selectedMode="local",this._certMode="auto",this._requestMicrophone()}async _requestMicrophone(){try{(await navigator.mediaDevices.getUserMedia({audio:!0})).getTracks().forEach(t=>t.stop()),this._micPermission="granted"}catch{this._micPermission="denied"}}async _runNetworkTests(){this._networkTestRunning=!0,this._networkTests=[{name:At("diag.wss",this.hass),status:"running"},{name:At("diag.stun",this.hass),status:"pending"},{name:At("diag.turn",this.hass),status:"pending"}],await this._testWss(),this._networkTests=this._networkTests.map((t,e)=>1===e?{...t,status:"running"}:t),await this._testStun(),this._networkTests=this._networkTests.map((t,e)=>2===e?{...t,status:"running"}:t),await this._testTurn(),this._networkTestRunning=!1}async _testWss(){const t=performance.now();try{if(this.hass?.connection?.socket?.readyState!==WebSocket.OPEN)throw new Error("Socket not open");{const e=Math.round(performance.now()-t);this._networkTests=this._networkTests.map((t,i)=>0===i?{...t,status:"pass",message:"WebSocket connected",durationMs:e}:t)}}catch{const e=Math.round(performance.now()-t);this._networkTests=this._networkTests.map((t,i)=>0===i?{...t,status:"fail",message:"WebSocket not available",durationMs:e}:t)}}async _testStun(){const t=performance.now();try{const e=new RTCPeerConnection({iceServers:[{urls:"stun:stun.l.google.com:19302"}]}),i=new Promise(t=>{const i=setTimeout(()=>t(!1),5e3);e.onicecandidate=e=>{e.candidate&&"srflx"===e.candidate.type&&(clearTimeout(i),t(!0))}});e.addTransceiver("audio",{direction:"sendrecv"});const s=await e.createOffer();await e.setLocalDescription(s);const a=await i;e.close();const o=Math.round(performance.now()-t);this._networkTests=this._networkTests.map((t,e)=>1===e?{...t,status:a?"pass":"warning",message:a?"STUN server reachable, server-reflexive candidates gathered":"No server-reflexive candidates (may be behind symmetric NAT)",durationMs:o}:t)}catch(e){const i=Math.round(performance.now()-t);this._networkTests=this._networkTests.map((t,s)=>1===s?{...t,status:"fail",message:String(e),durationMs:i}:t)}}async _testTurn(){const t=performance.now();try{if(this.hass){const e=await this.hass.callWS({type:"voip/diagnostics",test:"turn_credentials"});if(e?.urls?.length){const i=Math.round(performance.now()-t);return void(this._networkTests=this._networkTests.map((t,s)=>2===s?{...t,status:"pass",message:`TURN server configured: ${e.urls[0]}`,durationMs:i}:t))}}throw new Error("No TURN configuration available")}catch{const e=Math.round(performance.now()-t);this._networkTests=this._networkTests.map((t,i)=>2===i?{...t,status:"warning",message:"TURN not configured — calls may fail behind strict NAT",durationMs:e}:t)}}_addExtension(){const t=String(100+this._extensions.length);this._extensions=[...this._extensions,{name:"",number:t}]}_removeExtension(t){this._extensions=this._extensions.filter((e,i)=>i!==t)}_updateExtName(t,e){this._extensions=this._extensions.map((i,s)=>s===t?{...i,name:e}:i)}_updateExtNumber(t,e){this._extensions=this._extensions.map((i,s)=>s===t?{...i,number:e}:i)}async _startTestCall(){this._testCallState="testing";try{if(!this.hass)throw new Error("No HA connection");const t=await this.hass.callWS({type:"voip/onboarding",action:"test_call"});this._testCallState=t?.success?"success":"failure"}catch{this._testCallState="failure"}}async _finishSetup(){const t={mode:this._selectedMode,extensions:this._extensions.filter(t=>t.name&&t.number),certificateMode:this._certMode,certificatePath:"manual"===this._certMode?this._certPath:void 0,stunServers:["stun:stun.l.google.com:19302"],turnServers:[],completed:!0};if(this.hass)try{await this.hass.callWS({type:"voip/onboarding",action:"save_config",config:t})}catch(t){console.error("[Onboarding] Failed to save config:",t)}Lt(this,"voip-onboarding-complete",{config:t})}_renderStatusIcon(t){switch(t){case"pass":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z"/></svg>`;case"fail":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z"/></svg>`;case"warning":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M13,14H11V10H13M13,18H11V16H13M1,21H23L12,2L1,21Z"/></svg>`;case"running":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z"/></svg>`;default:return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2Z"/></svg>`}}};Dt.styles=[_t,vt,xt,kt,$t,Ct,n`
      :host {
        display: block;
      }

      .condensed-choice {
        display: flex;
        gap: 12px;
        margin-bottom: 16px;
      }

      .condensed-choice button {
        flex: 1;
      }

      .ext-row {
        display: flex;
        gap: 8px;
        align-items: center;
        margin-bottom: 8px;
      }

      .ext-row .form-input {
        flex: 1;
      }

      .ext-row .ext-number {
        width: 80px;
        flex: none;
      }

      .test-call-result {
        text-align: center;
        padding: 24px 0;
      }

      .test-call-result__icon {
        font-size: 48px;
        margin-bottom: 12px;
      }

      .test-call-result__message {
        font-size: 14px;
        color: var(--voip-secondary-text);
        line-height: 1.5;
      }

      .test-call-result--success .test-call-result__icon {
        color: var(--voip-success);
      }

      .test-call-result--failure .test-call-result__icon {
        color: var(--voip-error);
      }
    `],t([pt({attribute:!1})],Dt.prototype,"hass",void 0),t([ut()],Dt.prototype,"_currentStep",void 0),t([ut()],Dt.prototype,"_condensedMode",void 0),t([ut()],Dt.prototype,"_micPermission",void 0),t([ut()],Dt.prototype,"_networkTests",void 0),t([ut()],Dt.prototype,"_networkTestRunning",void 0),t([ut()],Dt.prototype,"_selectedMode",void 0),t([ut()],Dt.prototype,"_extensions",void 0),t([ut()],Dt.prototype,"_certMode",void 0),t([ut()],Dt.prototype,"_certPath",void 0),t([ut()],Dt.prototype,"_testCallState",void 0),Dt=t([ct("ha-voip-onboarding")],Dt);const Pt=["stun:stun.l.google.com:19302"];class Ot{constructor(t={},e={}){this._pc=null,this._localStream=null,this._remoteStream=null,this._hass=null,this._callId=null,this._statsTimer=null,this._audioLevelTimer=null,this._audioContext=null,this._analyser=null,this._reconnectAttempts=0,this._connectionState="new",this._isMuted=!1,this._unsubscribe=null,this._callbacks=t,this._config={stunServers:e.stunServers??Pt,turnServers:e.turnServers??[],audioConstraints:e.audioConstraints??{echoCancellation:!0,noiseSuppression:!0,autoGainControl:!0},enableStats:e.enableStats??!0,statsIntervalMs:e.statsIntervalMs??2e3,maxReconnectAttempts:e.maxReconnectAttempts??3,audioLevelIntervalMs:e.audioLevelIntervalMs??100}}setHass(t){this._hass=t}get connectionState(){return this._connectionState}get isMuted(){return this._isMuted}get peerConnection(){return this._pc}get localStream(){return this._localStream}get remoteStream(){return this._remoteStream}async enumerateAudioDevices(){return(await navigator.mediaDevices.enumerateDevices()).filter(t=>"audioinput"===t.kind||"audiooutput"===t.kind).map(t=>({deviceId:t.deviceId,label:t.label||`${t.kind} (${t.deviceId.slice(0,6)})`,kind:t.kind}))}async requestMicrophone(t){const e={audio:{...this._config.audioConstraints,...t?{deviceId:{exact:t}}:{}},video:!1};try{return await navigator.mediaDevices.getUserMedia(e)}catch(t){const e=t instanceof Error?t:new Error("Failed to access microphone");throw this._callbacks.onError?.(e),e}}async startCall(t,e){this._callId=t,this._reconnectAttempts=0,await this._createConnection(e);const i=await this._pc.createOffer();await this._pc.setLocalDescription(i),await this._sendWs({type:"voip/webrtc_offer",call_id:t,sdp:i.sdp}),await this._subscribeSignalling()}async answerCall(t,e,i){this._callId=t,this._reconnectAttempts=0,await this._createConnection(i),await this._pc.setRemoteDescription(new RTCSessionDescription({type:"offer",sdp:e}));const s=await this._pc.createAnswer();await this._pc.setLocalDescription(s),await this._sendWs({type:"voip/webrtc_answer",call_id:t,sdp:s.sdp}),await this._subscribeSignalling()}async handleRemoteAnswer(t){this._pc&&await this._pc.setRemoteDescription(new RTCSessionDescription({type:"answer",sdp:t}))}async addIceCandidate(t){if(this._pc)try{await this._pc.addIceCandidate(new RTCIceCandidate(t))}catch(t){console.warn("[WebRTC] Failed to add ICE candidate:",t)}}setMute(t){this._isMuted=t,this._localStream&&this._localStream.getAudioTracks().forEach(e=>{e.enabled=!t})}async switchAudioInput(t){if(!this._pc||!this._localStream)return;const e=(await this.requestMicrophone(t)).getAudioTracks()[0];if(!e)return;const i=this._pc.getSenders().find(t=>"audio"===t.track?.kind);i&&await i.replaceTrack(e),this._localStream.getAudioTracks().forEach(t=>t.stop()),this._localStream.removeTrack(this._localStream.getAudioTracks()[0]),this._localStream.addTrack(e),this._setupAudioLevelMonitor(),e.enabled=!this._isMuted}async setAudioOutput(t,e){"function"==typeof t.setSinkId&&await t.setSinkId(e)}async hangup(){if(this._stopTimers(),this._unsubscribe&&(this._unsubscribe(),this._unsubscribe=null),this._localStream&&(this._localStream.getTracks().forEach(t=>t.stop()),this._localStream=null),this._audioContext){try{await this._audioContext.close()}catch{}this._audioContext=null,this._analyser=null}this._pc&&(this._pc.close(),this._pc=null),this._remoteStream=null,this._callId=null,this._updateConnectionState("closed")}async getStats(){if(!this._pc)return null;try{const t=await this._pc.getStats();let e=0,i=0,s=0,a=0,o=0,n=0,r=0,l=0;return t.forEach(t=>{"inbound-rtp"===t.type&&"audio"===t.kind&&(e=t.bytesReceived??0,s=t.packetsReceived??0,o=t.packetsLost??0,n=t.jitter??0,l=t.audioLevel??0),"outbound-rtp"===t.type&&"audio"===t.kind&&(i=t.bytesSent??0,a=t.packetsSent??0),"candidate-pair"===t.type&&"succeeded"===t.state&&(r=t.currentRoundTripTime??0)}),{bytesReceived:e,bytesSent:i,packetsReceived:s,packetsSent:a,packetsLost:o,jitter:n,roundTripTime:r,audioLevel:l,timestamp:Date.now()}}catch{return null}}async gatherIceCandidates(t,e){const i=this._buildIceServers(t??this._config.stunServers,e??this._config.turnServers),s=new RTCPeerConnection({iceServers:i}),a=[];return new Promise(t=>{const e=setTimeout(()=>{s.close(),t(a)},1e4);s.onicecandidate=i=>{i.candidate?a.push(i.candidate):(clearTimeout(e),s.close(),t(a))},s.addTransceiver("audio",{direction:"sendrecv"}),s.createOffer().then(t=>s.setLocalDescription(t))})}_buildIceServers(t,e){const i=[];t.length>0&&i.push({urls:t});for(const t of e)i.push({urls:t.urls,username:t.username,credential:t.credential});return i}async _createConnection(t){this._localStream=await this.requestMicrophone(t);const e=this._buildIceServers(this._config.stunServers,this._config.turnServers);this._pc=new RTCPeerConnection({iceServers:e,iceCandidatePoolSize:2}),this._localStream.getTracks().forEach(t=>{this._pc.addTrack(t,this._localStream)}),this._pc.onicecandidate=t=>this._handleIceCandidate(t),this._pc.ontrack=t=>this._handleTrack(t),this._pc.onconnectionstatechange=()=>this._handleConnectionStateChange(),this._pc.oniceconnectionstatechange=()=>this._handleIceConnectionStateChange(),this._updateConnectionState("connecting"),this._setupAudioLevelMonitor(),this._config.enableStats&&this._startStatsPolling()}_handleIceCandidate(t){t.candidate&&this._callId&&(this._callbacks.onIceCandidate?.(t.candidate),this._sendWs({type:"voip/webrtc_candidate",call_id:this._callId,candidate:t.candidate.toJSON()}))}_handleTrack(t){t.streams[0]&&(this._remoteStream=t.streams[0],this._callbacks.onRemoteStream?.(this._remoteStream))}_handleConnectionStateChange(){if(!this._pc)return;const t=this._pc.connectionState;this._updateConnectionState(t),"failed"===t&&this._attemptReconnect()}_handleIceConnectionStateChange(){if(!this._pc)return;const t=this._pc.iceConnectionState;"connected"===t||"completed"===t?(this._updateConnectionState("connected"),this._reconnectAttempts=0):"disconnected"===t?this._updateConnectionState("disconnected"):"failed"===t&&this._attemptReconnect()}_updateConnectionState(t){t!==this._connectionState&&(this._connectionState=t,this._callbacks.onConnectionStateChange?.(t))}async _attemptReconnect(){if(this._reconnectAttempts>=this._config.maxReconnectAttempts)return void this._updateConnectionState("failed");this._reconnectAttempts++,this._callbacks.onReconnecting?.();const t=1e3*Math.pow(2,this._reconnectAttempts-1);if(await new Promise(e=>setTimeout(e,t)),this._pc&&this._callId)try{const t=await this._pc.createOffer({iceRestart:!0});await this._pc.setLocalDescription(t),await this._sendWs({type:"voip/webrtc_offer",call_id:this._callId,sdp:t.sdp}),this._callbacks.onReconnected?.()}catch(t){console.error("[WebRTC] Reconnection failed:",t),this._attemptReconnect()}}_setupAudioLevelMonitor(){if(this._localStream){this._audioLevelTimer&&(clearInterval(this._audioLevelTimer),this._audioLevelTimer=null);try{this._audioContext||(this._audioContext=new AudioContext);const t=this._audioContext.createMediaStreamSource(this._localStream);this._analyser=this._audioContext.createAnalyser(),this._analyser.fftSize=256,this._analyser.smoothingTimeConstant=.5,t.connect(this._analyser);const e=new Uint8Array(this._analyser.frequencyBinCount);this._audioLevelTimer=setInterval(()=>{if(!this._analyser)return;this._analyser.getByteFrequencyData(e);let t=0;for(let i=0;i<e.length;i++){const s=e[i]/255;t+=s*s}const i=Math.sqrt(t/e.length);this._callbacks.onAudioLevel?.(i)},this._config.audioLevelIntervalMs)}catch(t){console.warn("[WebRTC] Audio level monitoring unavailable:",t)}}}_startStatsPolling(){this._statsTimer=setInterval(async()=>{const t=await this.getStats();t&&this._callbacks.onStats?.(t)},this._config.statsIntervalMs)}_stopTimers(){this._statsTimer&&(clearInterval(this._statsTimer),this._statsTimer=null),this._audioLevelTimer&&(clearInterval(this._audioLevelTimer),this._audioLevelTimer=null)}async _sendWs(t){if(this._hass)try{await this._hass.callWS(t)}catch(t){console.error("[WebRTC] WS send error:",t),this._callbacks.onError?.(t instanceof Error?t:new Error("WebSocket send failed"))}else console.error("[WebRTC] No hass instance available")}async _subscribeSignalling(){if(this._hass&&this._callId)try{const t=await this._hass.connection.subscribeMessage(t=>{"webrtc_answer"===t.event&&t.call_id===this._callId?this.handleRemoteAnswer(t.sdp):"webrtc_candidate"===t.event&&t.call_id===this._callId&&this.addIceCandidate(t.candidate)},{type:"voip/subscribe",call_id:this._callId});this._unsubscribe=t}catch(t){console.error("[WebRTC] Failed to subscribe to signalling:",t)}}}let Nt=class extends rt{constructor(){super(...arguments),this._results=[],this._iceCandidates=[],this._isRunning=!1,this._networkRtt=null,this._showCandidates=!1,this._oneWayAudioResult=null,this._webrtc=new Ot}render(){return W`
      <!-- Header with actions -->
      <div class="diag-header">
        <h3 class="diag-header__title">${At("diag.title",this.hass)}</h3>
        <div class="diag-header__actions">
          <button
            class="diag-btn diag-btn--primary"
            ?disabled=${this._isRunning}
            @click=${this._runAllTests}
          >
            ${this._isRunning?W`<svg viewBox="0 0 24 24" width="14" height="14" style="animation:spin 1s linear infinite"><path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z"/></svg>`:q}
            ${At("diag.run_all",this.hass)}
          </button>
          <button
            class="diag-btn"
            @click=${this._exportJson}
            ?disabled=${0===this._results.length}
          >
            ${At("diag.export",this.hass)}
          </button>
        </div>
      </div>

      <!-- Test results -->
      <div class="diag-table">
        ${this._results.map(t=>W`
            <div class="diag-row">
              <div class="diag-icon diag-icon--${t.status}">
                ${this._renderStatusIcon(t.status)}
              </div>
              <div class="diag-info">
                <div class="diag-name">${t.name}</div>
                ${t.message?W`<div class="diag-message">${t.message}</div>`:q}
                ${t.details?W`<div class="diag-message" style="margin-top:4px;font-family:monospace;font-size:11px">${t.details}</div>`:q}
              </div>
              ${null!=t.durationMs?W`<div class="diag-time">${t.durationMs}ms</div>`:q}
            </div>
          `)}
      </div>

      <!-- RTT display -->
      ${null!=this._networkRtt?W`
            <div class="rtt-display">
              <div class="rtt-value">${this._networkRtt}<span style="font-size:14px">ms</span></div>
              <div class="rtt-label">${At("diag.rtt",this.hass)}</div>
            </div>
          `:q}

      <!-- ICE candidates section -->
      ${this._iceCandidates.length>0?W`
            <div class="candidates-section">
              <button class="candidates-toggle" @click=${()=>{this._showCandidates=!this._showCandidates}}>
                <svg viewBox="0 0 24 24" width="16" height="16" style="transform:rotate(${this._showCandidates?90:0}deg);transition:transform 0.2s">
                  <path fill="currentColor" d="M8.59,16.58L13.17,12L8.59,7.41L10,6L16,12L10,18L8.59,16.58Z"/>
                </svg>
                ${At("diag.ice_candidates",this.hass)} (${this._iceCandidates.length})
              </button>

              ${this._showCandidates?W`
                    <table class="candidates-table">
                      <thead>
                        <tr>
                          <th>Type</th>
                          <th>Protocol</th>
                          <th>Address</th>
                          <th>Port</th>
                          <th>Priority</th>
                        </tr>
                      </thead>
                      <tbody>
                        ${this._iceCandidates.map(t=>W`
                            <tr>
                              <td>${t.type}</td>
                              <td>${t.protocol}</td>
                              <td>${t.address}</td>
                              <td>${t.port}</td>
                              <td>${t.priority}</td>
                            </tr>
                          `)}
                      </tbody>
                    </table>
                  `:q}
            </div>
          `:q}

      <!-- One-way audio test -->
      <div class="one-way-audio-section">
        <button
          class="diag-btn"
          @click=${this._runOneWayAudioTest}
          ?disabled=${this._isRunning}
        >
          ${At("diag.one_way_audio",this.hass)}
        </button>
        ${this._oneWayAudioResult?W`<div class="one-way-audio-result">${this._oneWayAudioResult}</div>`:q}
      </div>
    `}async _runAllTests(){this._isRunning=!0,this._iceCandidates=[],this._networkRtt=null,this._results=[{name:At("diag.wss",this.hass),status:"running"},{name:At("diag.stun",this.hass),status:"pending"},{name:At("diag.turn",this.hass),status:"pending"},{name:At("diag.rtt",this.hass),status:"pending"}],await this._testWss(),this._updateResult(1,{status:"running"}),await this._testStun(),this._updateResult(2,{status:"running"}),await this._testTurn(),this._updateResult(3,{status:"running"}),await this._testRtt(),this._isRunning=!1}_updateResult(t,e){this._results=this._results.map((i,s)=>s===t?{...i,...e}:i)}async _testWss(){const t=performance.now();try{if(!this.hass?.connection?.socket)throw new Error("No HA WebSocket connection");const e=this.hass.connection.socket;if(e.readyState!==WebSocket.OPEN)throw new Error(`Socket state: ${e.readyState}`);const i=performance.now();await this.hass.callWS({type:"ping"});const s=Math.round(performance.now()-i),a=Math.round(performance.now()-t);this._updateResult(0,{status:"pass",message:`WebSocket connected (ping: ${s}ms)`,details:`URL: ${e.url}`,durationMs:a})}catch(e){const i=Math.round(performance.now()-t);this._updateResult(0,{status:"fail",message:`WebSocket test failed: ${e instanceof Error?e.message:String(e)}`,durationMs:i})}}async _testStun(){const t=performance.now();try{const e=await this._webrtc.gatherIceCandidates(["stun:stun.l.google.com:19302","stun:stun1.l.google.com:19302"],[]),i=e.filter(t=>t.candidate).map(t=>({type:t.type||"unknown",protocol:t.protocol||"unknown",address:t.address||"unknown",port:t.port||0,priority:t.priority||0,relatedAddress:t.relatedAddress||void 0,relatedPort:t.relatedPort||void 0}));this._iceCandidates=i;const s=i.some(t=>"srflx"===t.type),a=i.some(t=>"host"===t.type),o=Math.round(performance.now()-t);s?this._updateResult(1,{status:"pass",message:`Gathered ${e.length} candidates (${i.filter(t=>"srflx"===t.type).length} server-reflexive)`,durationMs:o}):a?this._updateResult(1,{status:"warning",message:`Only host candidates gathered (${e.length} total). May be behind symmetric NAT.`,durationMs:o}):this._updateResult(1,{status:"fail",message:"No ICE candidates gathered",durationMs:o})}catch(e){const i=Math.round(performance.now()-t);this._updateResult(1,{status:"fail",message:`STUN test failed: ${e instanceof Error?e.message:String(e)}`,durationMs:i})}}async _testTurn(){const t=performance.now();try{if(!this.hass)throw new Error("No HA connection");let e;try{e=await this.hass.callWS({type:"voip/diagnostics",test:"turn_credentials"})}catch{throw new Error("Backend did not provide TURN credentials")}if(!e?.urls?.length)throw new Error("No TURN server URLs configured");const i=(await this._webrtc.gatherIceCandidates([],[{urls:e.urls,username:e.username,credential:e.credential}])).filter(t=>"relay"===t.type),s=Math.round(performance.now()-t);if(i.length>0){const t=i.map(t=>({type:"relay",protocol:t.protocol||"unknown",address:t.address||"unknown",port:t.port||0,priority:t.priority||0,relatedAddress:t.relatedAddress||void 0,relatedPort:t.relatedPort||void 0}));this._iceCandidates=[...this._iceCandidates,...t],this._updateResult(2,{status:"pass",message:`TURN allocation succeeded (${i.length} relay candidates)`,details:`Server: ${e.urls[0]}`,durationMs:s})}else this._updateResult(2,{status:"fail",message:"TURN allocation failed — no relay candidates obtained",details:`Server: ${e.urls[0]}`,durationMs:s})}catch(e){const i=Math.round(performance.now()-t);this._updateResult(2,{status:"warning",message:e instanceof Error?e.message:String(e),durationMs:i})}}async _testRtt(){const t=performance.now();try{if(!this.hass)throw new Error("No HA connection");const e=[];for(let t=0;t<5;t++){const t=performance.now();await this.hass.callWS({type:"ping"}),e.push(performance.now()-t)}e.sort((t,e)=>t-e);const i=e.length>2?e.slice(1,-1):e,s=Math.round(i.reduce((t,e)=>t+e,0)/i.length);this._networkRtt=s;const a=Math.round(performance.now()-t);let o="pass",n=`Average RTT: ${s}ms (${e.length} samples)`;s>300?(o="fail",n+=" — latency is too high for real-time voice"):s>150&&(o="warning",n+=" — latency may cause noticeable delay"),this._updateResult(3,{status:o,message:n,durationMs:a})}catch(e){const i=Math.round(performance.now()-t);this._updateResult(3,{status:"fail",message:`RTT test failed: ${e instanceof Error?e.message:String(e)}`,durationMs:i})}}async _runOneWayAudioTest(){this._oneWayAudioResult=null;try{const t=await navigator.mediaDevices.getUserMedia({audio:!0}),e=new AudioContext,i=e.createMediaStreamSource(t),s=e.createAnalyser();s.fftSize=256,i.connect(s);const a=new Uint8Array(s.frequencyBinCount);let o=0,n=0;const r=3e3,l=50;await new Promise(t=>{const e=setInterval(()=>{s.getByteFrequencyData(a);let i=0;for(let t=0;t<a.length;t++)i+=a[t];const c=i/a.length;c>o&&(o=c),n++,n>=r/l&&(clearInterval(e),t())},l)}),t.getTracks().forEach(t=>t.stop()),await e.close(),this._oneWayAudioResult=o>30?`Microphone is working. Peak audio level: ${Math.round(o)}/255. Speak to verify your voice is being captured.`:o>5?`Microphone detected low audio. Peak level: ${Math.round(o)}/255. Check your microphone volume.`:`No audio detected (peak: ${Math.round(o)}/255). The microphone may be muted or not working.`}catch(t){this._oneWayAudioResult=`Audio test failed: ${t instanceof Error?t.message:String(t)}`}}_exportJson(){const t={timestamp:Date.now(),userAgent:navigator.userAgent,results:this._results,iceCandidates:this._iceCandidates,networkRtt:this._networkRtt??void 0},e=new Blob([JSON.stringify(t,null,2)],{type:"application/json"}),i=URL.createObjectURL(e),s=document.createElement("a");s.href=i,s.download=`ha-voip-diagnostics-${(new Date).toISOString().slice(0,10)}.json`,s.click(),URL.revokeObjectURL(i)}_renderStatusIcon(t){switch(t){case"pass":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M21,7L9,19L3.5,13.5L4.91,12.09L9,16.17L19.59,5.59L21,7Z"/></svg>`;case"fail":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z"/></svg>`;case"warning":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M13,14H11V10H13M13,18H11V16H13M1,21H23L12,2L1,21Z"/></svg>`;case"running":return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M12,4V2A10,10 0 0,0 2,12H4A8,8 0 0,1 12,4Z"/></svg>`;default:return W`<svg viewBox="0 0 24 24" width="16" height="16"><path fill="currentColor" d="M12,20A8,8 0 0,1 4,12A8,8 0 0,1 12,4A8,8 0 0,1 20,12A8,8 0 0,1 12,20M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2Z"/></svg>`}}};Nt.styles=[_t,vt,$t,Ct,n`
      :host {
        display: block;
      }

      .diag-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 16px;
        border-bottom: 1px solid var(--voip-divider);
      }

      .diag-header__title {
        font-size: 16px;
        font-weight: 500;
        margin: 0;
      }

      .diag-header__actions {
        display: flex;
        gap: 8px;
      }

      .diag-btn {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 6px 14px;
        border: 1px solid var(--voip-divider);
        border-radius: 8px;
        background: none;
        font-size: 13px;
        font-family: inherit;
        color: var(--voip-primary-text);
        cursor: pointer;
        transition: background-color 0.15s;
      }

      .diag-btn:hover {
        background-color: rgba(0, 0, 0, 0.04);
      }

      .diag-btn--primary {
        background-color: var(--voip-primary);
        border-color: var(--voip-primary);
        color: #fff;
      }

      .diag-btn--primary:hover {
        filter: brightness(1.1);
      }

      .diag-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
      }

      .candidates-section {
        padding: 12px 16px;
        border-top: 1px solid var(--voip-divider);
      }

      .candidates-toggle {
        display: flex;
        align-items: center;
        gap: 8px;
        cursor: pointer;
        background: none;
        border: none;
        font-size: 13px;
        font-weight: 500;
        color: var(--voip-primary);
        padding: 0;
        font-family: inherit;
      }

      .candidates-table {
        width: 100%;
        margin-top: 8px;
        font-size: 12px;
        border-collapse: collapse;
      }

      .candidates-table th {
        text-align: left;
        padding: 6px 8px;
        color: var(--voip-secondary-text);
        font-weight: 500;
        border-bottom: 1px solid var(--voip-divider);
      }

      .candidates-table td {
        padding: 6px 8px;
        border-bottom: 1px solid var(--voip-divider);
        font-family: monospace;
        font-size: 11px;
      }

      .rtt-display {
        text-align: center;
        padding: 16px;
        border-top: 1px solid var(--voip-divider);
      }

      .rtt-value {
        font-size: 32px;
        font-weight: 300;
        color: var(--voip-primary);
      }

      .rtt-label {
        font-size: 12px;
        color: var(--voip-secondary-text);
        margin-top: 4px;
      }

      .one-way-audio-section {
        padding: 12px 16px;
        border-top: 1px solid var(--voip-divider);
      }

      .one-way-audio-result {
        margin-top: 8px;
        padding: 8px 12px;
        border-radius: 6px;
        font-size: 13px;
        background-color: rgba(0, 0, 0, 0.04);
      }
    `],t([pt({attribute:!1})],Nt.prototype,"hass",void 0),t([ut()],Nt.prototype,"_results",void 0),t([ut()],Nt.prototype,"_iceCandidates",void 0),t([ut()],Nt.prototype,"_isRunning",void 0),t([ut()],Nt.prototype,"_networkRtt",void 0),t([ut()],Nt.prototype,"_showCandidates",void 0),t([ut()],Nt.prototype,"_oneWayAudioResult",void 0),Nt=t([ct("ha-voip-diagnostics")],Nt);let Ut=class extends rt{setConfig(t){this._config=t}render(){return this._config?W`
      <div class="editor">
        <div class="editor-row">
          <label>${At("config.card_title",this.hass)}</label>
          <input
            type="text"
            .value=${this._config.title||""}
            @input=${t=>this._update("title",t.target.value)}
          />
        </div>

        <div class="editor-row">
          <label>${At("config.entity",this.hass)}</label>
          <input
            type="text"
            .value=${this._config.entity||""}
            placeholder="sensor.voip_status"
            @input=${t=>this._update("entity",t.target.value)}
          />
        </div>

        <div class="editor-toggle">
          <label>${At("config.show_dialpad",this.hass)}</label>
          <input
            type="checkbox"
            .checked=${!1!==this._config.show_dialpad}
            @change=${t=>this._update("show_dialpad",t.target.checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${At("config.show_recent",this.hass)}</label>
          <input
            type="checkbox"
            .checked=${!1!==this._config.show_recent_calls}
            @change=${t=>this._update("show_recent_calls",t.target.checked)}
          />
        </div>

        <div class="editor-row">
          <label>${At("config.recent_count",this.hass)}</label>
          <input
            type="number"
            min="1"
            max="20"
            .value=${String(this._config.recent_calls_count??5)}
            @input=${t=>this._update("recent_calls_count",parseInt(t.target.value)||5)}
          />
        </div>

        <div class="editor-toggle">
          <label>${At("config.show_diagnostics",this.hass)}</label>
          <input
            type="checkbox"
            .checked=${!0===this._config.show_diagnostics}
            @change=${t=>this._update("show_diagnostics",t.target.checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${At("config.compact_mode",this.hass)}</label>
          <input
            type="checkbox"
            .checked=${!0===this._config.compact_mode}
            @change=${t=>this._update("compact_mode",t.target.checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${At("config.enable_dtmf",this.hass)}</label>
          <input
            type="checkbox"
            .checked=${!1!==this._config.enable_dtmf_tones}
            @change=${t=>this._update("enable_dtmf_tones",t.target.checked)}
          />
        </div>

        <div class="editor-toggle">
          <label>${At("config.auto_answer",this.hass)}</label>
          <input
            type="checkbox"
            .checked=${!0===this._config.auto_answer}
            @change=${t=>this._update("auto_answer",t.target.checked)}
          />
        </div>

        <!-- Quick dial entries -->
        <div class="editor-row">
          <label>${At("config.quick_dial",this.hass)}</label>
          ${(this._config.quick_dial||[]).map((t,e)=>W`
              <div class="quick-dial-entry">
                <input
                  type="text"
                  placeholder=${At("config.name",this.hass)}
                  .value=${t.name}
                  @input=${t=>this._updateQuickDial(e,"name",t.target.value)}
                />
                <input
                  type="tel"
                  placeholder=${At("config.number",this.hass)}
                  .value=${t.number}
                  @input=${t=>this._updateQuickDial(e,"number",t.target.value)}
                />
                <button class="remove-btn" @click=${()=>this._removeQuickDial(e)}>
                  <svg viewBox="0 0 24 24" width="16" height="16">
                    <path fill="currentColor" d="M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z" />
                  </svg>
                </button>
              </div>
            `)}
          <button class="add-btn" @click=${this._addQuickDial}>
            + ${At("config.add_quick_dial",this.hass)}
          </button>
        </div>
      </div>
    `:q}_update(t,e){this._config&&(this._config={...this._config,[t]:e},Lt(this,"config-changed",{config:this._config}))}_updateQuickDial(t,e,i){const s=[...this._config?.quick_dial||[]];s[t]={...s[t],[e]:i},this._update("quick_dial",s)}_addQuickDial(){const t=[...this._config?.quick_dial||[],{name:"",number:""}];this._update("quick_dial",t)}_removeQuickDial(t){const e=(this._config?.quick_dial||[]).filter((e,i)=>i!==t);this._update("quick_dial",e)}};Ut.styles=[_t,n`
      .editor {
        padding: 16px;
      }
      .editor-row {
        margin-bottom: 12px;
      }
      .editor-row label {
        display: block;
        font-size: 13px;
        font-weight: 500;
        color: var(--voip-secondary-text, #727272);
        margin-bottom: 4px;
      }
      .editor-row input,
      .editor-row select {
        width: 100%;
        padding: 8px 10px;
        border: 1px solid var(--voip-divider, rgba(0,0,0,0.12));
        border-radius: 6px;
        font-size: 14px;
        font-family: inherit;
        background: var(--voip-surface, #fff);
        color: var(--voip-primary-text, #212121);
      }
      .editor-row input:focus,
      .editor-row select:focus {
        outline: none;
        border-color: var(--voip-primary, #03a9f4);
      }
      .editor-toggle {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 8px 0;
      }
      .editor-toggle label {
        margin: 0;
      }
      .quick-dial-entry {
        display: flex;
        gap: 8px;
        margin-bottom: 6px;
        align-items: center;
      }
      .quick-dial-entry input {
        flex: 1;
      }
      .add-btn {
        font-size: 13px;
        color: var(--voip-primary);
        cursor: pointer;
        background: none;
        border: none;
        padding: 4px 0;
        font-family: inherit;
      }
      .remove-btn {
        width: 28px;
        height: 28px;
        border-radius: 50%;
        border: none;
        background: none;
        cursor: pointer;
        color: var(--voip-error, #db4437);
        display: flex;
        align-items: center;
        justify-content: center;
      }
    `],t([pt({attribute:!1})],Ut.prototype,"hass",void 0),t([ut()],Ut.prototype,"_config",void 0),Ut=t([ct("ha-voip-card-editor")],Ut);let Bt=class extends rt{constructor(){super(),this._callState=null,this._extensions=[],this._history=[],this._showPopup=!1,this._showDialpad=!1,this._showDiagnostics=!1,this._showOnboarding=!1,this._view="main",this._remoteAudio=null,this._ringtoneAudio=null,this._webrtc=new Ot({onConnectionStateChange:t=>{"connected"===t&&this._stopRingtone()},onRemoteStream:t=>{this._attachRemoteAudio(t)},onError:t=>{console.error("[VoIP Card] WebRTC error:",t)}})}static getConfigElement(){return document.createElement("ha-voip-card-editor")}static getStubConfig(){return{type:"custom:ha-voip-card",title:"VoIP Phone",show_recent_calls:!0,recent_calls_count:5,show_dialpad:!0,enable_dtmf_tones:!0}}setConfig(t){if(!t)throw new Error(At("card.no_config"));this._config={show_recent_calls:!0,recent_calls_count:5,show_dialpad:!0,enable_dtmf_tones:!0,...t}}getCardSize(){let t=3;return!1!==this._config?.show_dialpad&&(t+=5),!1!==this._config?.show_recent_calls&&(t+=3),this._config?.quick_dial?.length&&(t+=1),t}connectedCallback(){super.connectedCallback(),this._createRemoteAudio()}disconnectedCallback(){super.disconnectedCallback(),this._unsubscribeWs(),this._webrtc.hangup(),this._destroyAudioElements()}updated(t){super.updated(t),t.has("hass")&&this.hass&&(this._webrtc.setHass(this.hass),this._unsubscribe||this._subscribeWs())}render(){return this._config?W`
      <ha-card>
        ${this._renderHeader()}
        ${this._renderContent()}
      </ha-card>

      <!-- Call popup overlay -->
      ${this._showPopup&&this._callState?W`
            <ha-voip-call-popup
              .hass=${this.hass}
              .callState=${this._callState}
              .cameraEntityId=${this._incomingCameraEntity}
              @voip-answer=${this._handleAnswer}
              @voip-hangup=${this._handleHangup}
              @voip-mute=${this._handleMute}
              @voip-hold=${this._handleHold}
              @voip-speaker=${this._handleSpeaker}
              @voip-record=${this._handleRecord}
              @voip-transfer-start=${this._handleTransferStart}
              @voip-dtmf=${this._handleDtmf}
              @voip-device-change=${this._handleDeviceChange}
              @voip-popup-minimize=${()=>{this._showPopup=!1}}
            ></ha-voip-call-popup>
          `:q}

      <!-- Onboarding wizard overlay -->
      ${this._showOnboarding?W`
            <div class="popup-overlay" style="position:fixed;inset:0;z-index:1001;display:flex;align-items:center;justify-content:center;background:rgba(0,0,0,0.6)">
              <div style="background:var(--voip-surface,#fff);border-radius:var(--voip-radius,12px);max-width:480px;width:90vw;max-height:90vh;overflow-y:auto">
                <ha-voip-onboarding
                  .hass=${this.hass}
                  @voip-onboarding-complete=${this._handleOnboardingComplete}
                ></ha-voip-onboarding>
              </div>
            </div>
          `:q}
    `:W`<ha-card><div class="empty-state">${At("card.no_config")}</div></ha-card>`}_renderHeader(){const t=this._config?.title||At("card.title",this.hass);return W`
      <div class="card-header">
        <h2 class="card-title">${t}</h2>
        <div class="card-header__actions">
          ${this._config?.show_diagnostics?W`
                <button
                  class="btn btn--sm btn--icon"
                  @click=${()=>{this._view="diagnostics"===this._view?"main":"diagnostics"}}
                  aria-label=${At("diag.title",this.hass)}
                >
                  <svg viewBox="0 0 24 24" width="20" height="20">
                    <path fill="currentColor" d="M12,15.5A3.5,3.5 0 0,1 8.5,12A3.5,3.5 0 0,1 12,8.5A3.5,3.5 0 0,1 15.5,12A3.5,3.5 0 0,1 12,15.5M19.43,12.97C19.47,12.65 19.5,12.33 19.5,12C19.5,11.67 19.47,11.34 19.43,11L21.54,9.37C21.73,9.22 21.78,8.95 21.66,8.73L19.66,5.27C19.54,5.05 19.27,4.96 19.05,5.05L16.56,6.05C16.04,5.66 15.5,5.32 14.87,5.07L14.5,2.42C14.46,2.18 14.25,2 14,2H10C9.75,2 9.54,2.18 9.5,2.42L9.13,5.07C8.5,5.32 7.96,5.66 7.44,6.05L4.95,5.05C4.73,4.96 4.46,5.05 4.34,5.27L2.34,8.73C2.21,8.95 2.27,9.22 2.46,9.37L4.57,11C4.53,11.34 4.5,11.67 4.5,12C4.5,12.33 4.53,12.65 4.57,12.97L2.46,14.63C2.27,14.78 2.21,15.05 2.34,15.27L4.34,18.73C4.46,18.95 4.73,19.03 4.95,18.95L7.44,17.94C7.96,18.34 8.5,18.68 9.13,18.93L9.5,21.58C9.54,21.82 9.75,22 10,22H14C14.25,22 14.46,21.82 14.5,21.58L14.87,18.93C15.5,18.67 16.04,18.34 16.56,17.94L19.05,18.95C19.27,19.03 19.54,18.95 19.66,18.73L21.66,15.27C21.78,15.05 21.73,14.78 21.54,14.63L19.43,12.97Z" />
                  </svg>
                </button>
              `:q}
          <button
            class="btn btn--sm btn--icon"
            @click=${()=>{this._showOnboarding=!0}}
            aria-label="Setup"
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M11,9H13V7H11M12,20C7.59,20 4,16.41 4,12C4,7.59 7.59,4 12,4C16.41,4 20,7.59 20,12C20,16.41 16.41,20 12,20M12,2A10,10 0 0,0 2,12A10,10 0 0,0 12,22A10,10 0 0,0 22,12A10,10 0 0,0 12,2M11,17H13V11H11V17Z" />
            </svg>
          </button>
        </div>
      </div>
    `}_renderContent(){return"diagnostics"===this._view?W`
          <ha-voip-diagnostics .hass=${this.hass}></ha-voip-diagnostics>
        `:this._renderMainView()}_renderMainView(){const t=this._callState&&"idle"!==this._callState.state&&"ended"!==this._callState.state;return W`
      <!-- Extension info -->
      ${this._renderExtensionInfo()}

      <!-- Active call banner -->
      ${t?this._renderActiveCallBanner():q}

      <!-- Call status display -->
      ${this._callState&&"idle"!==this._callState.state?this._renderCallStatus():q}

      <!-- Active call controls -->
      ${t?this._renderCallControls():q}

      <!-- Quick dial -->
      ${this._config?.quick_dial?.length?this._renderQuickDial():q}

      <!-- Nav tabs: Dialpad / Recent -->
      ${this._renderTabs()}

      <!-- Tab content -->
      ${this._showDialpad?W`
            <div class="dialpad-section">
              <ha-voip-dialpad
                .hass=${this.hass}
                .callState=${this._callState?.state??"idle"}
                .enableDtmf=${!1!==this._config?.enable_dtmf_tones}
                @voip-call=${this._handleDialCall}
                @voip-hangup=${this._handleHangup}
                @voip-dtmf=${this._handleDtmf}
              ></ha-voip-dialpad>
            </div>
          `:this._renderRecentCalls()}
    `}_renderExtensionInfo(){const t=this._extensions.find(t=>t.userId===this.hass?.user?.id)||this._extensions[0];if(!t)return W`
        <div class="extension-info">
          <div class="extension-avatar">
            <svg viewBox="0 0 24 24" width="24" height="24">
              <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
            </svg>
          </div>
          <div class="extension-details">
            <div class="extension-name">${this.hass?.user?.name||"VoIP"}</div>
            <div class="extension-status">
              <span class="status-dot status-dot--offline"></span>
              <span>${At("ext.offline",this.hass)}</span>
            </div>
          </div>
        </div>
      `;const e=t.name.split(" ").map(t=>t[0]).join("").slice(0,2).toUpperCase();return W`
      <div class="extension-info">
        <div class="extension-avatar">${e}</div>
        <div class="extension-details">
          <div class="extension-name">${t.name}</div>
          <div class="extension-number">Ext. ${t.number}</div>
          <div class="extension-status">
            <span class="status-dot status-dot--${t.status}"></span>
            <span>${At(`ext.${t.status}`,this.hass)}</span>
          </div>
        </div>
        <span class="badge badge--${this._callState?.state||"idle"}">
          ${At(`call.${this._callState?.state||"idle"}`,this.hass)}
        </span>
      </div>
    `}_renderActiveCallBanner(){if(!this._callState)return q;const t=this._callState.remoteName||this._callState.remoteNumber,e=this._callState.connectTime?Math.floor((Date.now()-this._callState.connectTime)/1e3):0;return W`
      <div class="active-call-banner" @click=${()=>{this._showPopup=!0}}>
        <div class="active-call-info">
          <svg viewBox="0 0 24 24" width="18" height="18" style="color:var(--voip-success)">
            <path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z" />
          </svg>
          <span class="active-call-name">${t}</span>
        </div>
        <span class="active-call-timer">${Mt(e)}</span>
      </div>
    `}_renderCallStatus(){return q}_renderCallControls(){if(!this._callState)return q;const t=this._callState;return W`
      <div class="call-controls">
        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${t.isMuted?"active":""}"
            @click=${()=>this._handleMute(new CustomEvent("voip-mute",{detail:{mute:!t.isMuted}}))}
            aria-label=${t.isMuted?At("controls.unmute",this.hass):At("controls.mute",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              ${t.isMuted?W`<path fill="currentColor" d="M19,11C19,12.19 18.66,13.3 18.1,14.28L16.87,13.05C17.14,12.43 17.3,11.74 17.3,11H19M15,11.16L9,5.18V5A3,3 0 0,1 12,2A3,3 0 0,1 15,5V11L15,11.16M4.27,3L3,4.27L9.01,10.28V11A3,3 0 0,0 12.01,14C12.22,14 12.42,13.97 12.62,13.92L14.01,15.31C13.39,15.6 12.72,15.78 12.01,15.83V19H14.01V21H10.01V19H12.01V15.83C9.24,15.56 7.01,13.5 7.01,11H8.71C8.71,13 10.41,14.29 12.01,14.29C12.33,14.29 12.63,14.24 12.92,14.15L11.51,12.74C11.35,12.77 11.18,12.8 11.01,12.8A1.8,1.8 0 0,1 9.21,11V10.28L4.27,3Z" />`:W`<path fill="currentColor" d="M12,2A3,3 0 0,1 15,5V11A3,3 0 0,1 12,14A3,3 0 0,1 9,11V5A3,3 0 0,1 12,2M19,11C19,14.53 16.39,17.44 13,17.93V21H11V17.93C7.61,17.44 5,14.53 5,11H7A5,5 0 0,0 12,16A5,5 0 0,0 17,11H19Z" />`}
            </svg>
          </button>
          <span>${t.isMuted?At("controls.unmute",this.hass):At("controls.mute",this.hass)}</span>
        </div>

        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action ${t.isOnHold?"active":""}"
            @click=${()=>this._handleHold(new CustomEvent("voip-hold",{detail:{hold:!t.isOnHold}}))}
            aria-label=${At("controls.hold",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M14,19H18V5H14M6,19H10V5H6V19Z" />
            </svg>
          </button>
          <span>${t.isOnHold?At("controls.unhold",this.hass):At("controls.hold",this.hass)}</span>
        </div>

        <div class="call-controls__label">
          <button
            class="btn btn--md btn--hangup"
            @click=${()=>this._handleHangup()}
            aria-label=${At("controls.hangup",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M12,9C10.4,9 8.85,9.25 7.4,9.72V12.82C7.4,13.22 7.17,13.56 6.84,13.72C5.86,14.21 4.97,14.84 4.17,15.57C4,15.75 3.75,15.86 3.5,15.86C3.2,15.86 2.95,15.74 2.77,15.56L0.29,13.08C0.11,12.9 0,12.65 0,12.38C0,12.1 0.11,11.85 0.29,11.67C3.34,8.77 7.46,7 12,7C16.54,7 20.66,8.77 23.71,11.67C23.89,11.85 24,12.1 24,12.38C24,12.65 23.89,12.9 23.71,13.08L21.23,15.56C21.05,15.74 20.8,15.86 20.5,15.86C20.25,15.86 20,15.75 19.83,15.57C19.03,14.84 18.14,14.21 17.16,13.72C16.83,13.56 16.6,13.22 16.6,12.82V9.72C15.15,9.25 13.6,9 12,9Z" />
            </svg>
          </button>
          <span>${At("controls.hangup",this.hass)}</span>
        </div>

        <div class="call-controls__label">
          <button
            class="btn btn--md btn--action"
            @click=${this._handleTransferStart}
            aria-label=${At("controls.transfer",this.hass)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path fill="currentColor" d="M18,13V5H20V13H18M14,5V13H16V5H14M11,5L6,10L11,15V12C15.39,12 19.17,13.58 22,16.28C20.63,11.11 16.33,7.15 11,6.34V5Z" />
            </svg>
          </button>
          <span>${At("controls.transfer",this.hass)}</span>
        </div>
      </div>
    `}_renderQuickDial(){const t=this._config?.quick_dial||[];return W`
      <div class="quick-dial-section">
        <p class="quick-dial-title">${At("quickdial.title",this.hass)}</p>
        <div class="quick-dial-grid">
          ${t.map(t=>W`
              <button
                class="quick-dial-chip"
                @click=${()=>this._dialNumber(t.number)}
                aria-label="${t.name} (${t.number})"
              >
                ${t.icon?W`<ha-icon icon=${t.icon}></ha-icon>`:q}
                ${t.name}
              </button>
            `)}
        </div>
      </div>
    `}_renderTabs(){return this._config?.compact_mode?q:W`
      <div class="nav-tabs">
        <button
          class="nav-tab ${this._showDialpad?"nav-tab--active":""}"
          @click=${()=>{this._showDialpad=!0}}
          ?hidden=${!1===this._config?.show_dialpad}
        >
          ${At("dialpad.title",this.hass)}
        </button>
        <button
          class="nav-tab ${this._showDialpad?"":"nav-tab--active"}"
          @click=${()=>{this._showDialpad=!1}}
          ?hidden=${!1===this._config?.show_recent_calls}
        >
          ${At("history.title",this.hass)}
        </button>
      </div>
    `}_renderRecentCalls(){if(!1===this._config?.show_recent_calls)return q;const t=this._config?.recent_calls_count??5,e=this._history.slice(0,t);return 0===e.length?W`<div class="empty-state">${At("history.no_calls",this.hass)}</div>`:W`
      <ul class="history-list">
        ${e.map(t=>W`
            <li
              class="history-item"
              @click=${()=>this._dialNumber(t.remoteNumber)}
              tabindex="0"
              role="button"
              aria-label="${t.remoteName||t.remoteNumber}"
            >
              <div class="history-item__icon ${this._historyIconClass(t)}">
                ${this._historyIcon(t)}
              </div>
              <div class="history-item__info">
                <div class="history-item__name">
                  ${t.remoteName||t.remoteNumber}
                </div>
                <div class="history-item__number">
                  ${t.remoteName?t.remoteNumber:""}
                </div>
              </div>
              <div class="history-item__meta">
                <div class="history-item__time">
                  ${function(t){const e=Date.now()-t,i=Math.floor(e/1e3),s=Math.floor(i/60),a=Math.floor(s/60),o=Math.floor(a/24);return o>1?new Date(t).toLocaleDateString():1===o?St["history.yesterday"]:a>0?`${a}h ago`:s>0?`${s}m ago`:"Just now"}(t.startTime)}
                </div>
                <div class="history-item__duration">
                  ${t.answered?Mt(t.duration):At("history.missed",this.hass)}
                </div>
              </div>
            </li>
          `)}
      </ul>
    `}_historyIconClass(t){return t.answered?"inbound"===t.direction?"history-item__icon--inbound":"history-item__icon--outbound":"history-item__icon--missed"}_historyIcon(t){return t.answered?"inbound"===t.direction?W`<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M20,5.41L18.59,4L7,15.59V9H5V19H15V17H8.41L20,5.41Z"/></svg>`:W`<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M4,18.59L5.41,20L17,8.41V15H19V5H9V7H15.59L4,18.59Z"/></svg>`:W`<svg viewBox="0 0 24 24" width="18" height="18"><path fill="currentColor" d="M6.62,10.79C8.06,13.62 10.38,15.94 13.21,17.38L15.41,15.18C15.69,14.9 16.08,14.82 16.43,14.93C17.55,15.3 18.75,15.5 20,15.5A1,1 0 0,1 21,16.5V20A1,1 0 0,1 20,21A17,17 0 0,1 3,4A1,1 0 0,1 4,3H7.5A1,1 0 0,1 8.5,4C8.5,5.25 8.7,6.45 9.07,7.57C9.18,7.92 9.1,8.31 8.82,8.59L6.62,10.79Z"/></svg>`}async _subscribeWs(){if(this.hass)try{const t=await this.hass.connection.subscribeMessage(t=>this._handleVoipEvent(t),{type:"voip/subscribe"});this._unsubscribe=t,this._fetchExtensions(),this._fetchHistory()}catch(t){console.error("[VoIP Card] Failed to subscribe:",t)}}_unsubscribeWs(){this._unsubscribe&&(this._unsubscribe(),this._unsubscribe=void 0)}_handleVoipEvent(t){switch(t.event){case"call_state":this._callState=t.data,"ringing"===t.data.state&&"inbound"===t.data.direction?(this._showPopup=!0,this._playRingtone(),this._config?.auto_answer&&setTimeout(()=>{this._handleAnswer()},1e3)):"ended"===t.data.state&&(this._stopRingtone(),setTimeout(()=>{"ended"===this._callState?.state&&(this._showPopup=!1,this._callState=null)},2e3),this._fetchHistory());break;case"extensions":this._extensions=t.data;break;case"history":this._history=t.data;break;case"incoming_call":this._incomingCameraEntity=t.data.camera_entity_id;break;case"webrtc_offer":this._webrtc.answerCall(t.call_id,t.sdp);break;case"webrtc_answer":this._webrtc.handleRemoteAnswer(t.sdp);break;case"webrtc_candidate":this._webrtc.addIceCandidate(t.candidate)}}async _fetchExtensions(){if(this.hass)try{const t=await this.hass.callWS({type:"voip/extensions"});t&&(this._extensions=t)}catch{}}async _fetchHistory(){if(this.hass)try{const t=await this.hass.callWS({type:"voip/history"});t&&(this._history=t)}catch{}}_dialNumber(t){this.hass&&t&&this.hass.callWS({type:"voip/call",number:t}).then(t=>{t?.call_id&&this._webrtc.startCall(t.call_id)})}_handleDialCall(t){this._dialNumber(t.detail.number)}async _handleAnswer(t){if(!this.hass||!this._callState)return;const e=t?.detail?.call_id||this._callState.id;this._stopRingtone(),await this.hass.callWS({type:"voip/answer",call_id:e})}async _handleHangup(t){if(!this.hass)return;const e=t?.detail?.call_id||this._callState?.id;e&&await this.hass.callWS({type:"voip/hangup",call_id:e}),await this._webrtc.hangup(),this._stopRingtone()}async _handleMute(t){if(!this.hass||!this._callState)return;const e=t.detail?.mute??!this._callState.isMuted;this._webrtc.setMute(e),await this.hass.callWS({type:"voip/mute",call_id:this._callState.id,mute:e})}async _handleHold(t){if(!this.hass||!this._callState)return;const e=t.detail?.hold??!this._callState.isOnHold;await this.hass.callWS({type:"voip/hold",call_id:this._callState.id,hold:e})}async _handleSpeaker(t){this._callState&&(this._callState={...this._callState,isSpeaker:t.detail?.speaker??!this._callState.isSpeaker})}async _handleRecord(t){if(!this.hass||!this._callState)return;const e=t.detail?.record??!this._callState.isRecording;await this.hass.callWS({type:"voip/record",call_id:this._callState.id,record:e})}_handleTransferStart(){const t=prompt(At("controls.transfer",this.hass));t&&this.hass&&this._callState&&this.hass.callWS({type:"voip/transfer",call_id:this._callState.id,target:t})}async _handleDtmf(t){this.hass&&this._callState&&await this.hass.callWS({type:"voip/dtmf",call_id:this._callState.id,digit:t.detail.digit})}async _handleDeviceChange(t){const{deviceId:e,kind:i}=t.detail;"audioinput"===i?await this._webrtc.switchAudioInput(e):"audiooutput"===i&&this._remoteAudio&&await this._webrtc.setAudioOutput(this._remoteAudio,e)}_handleOnboardingComplete(t){this._showOnboarding=!1}_createRemoteAudio(){this._remoteAudio=document.createElement("audio"),this._remoteAudio.autoplay=!0,this._remoteAudio.playsInline=!0,document.body.appendChild(this._remoteAudio)}_attachRemoteAudio(t){this._remoteAudio&&(this._remoteAudio.srcObject=t)}_playRingtone(){const t=this._config?.ringtone_url;if(t)try{this._ringtoneAudio=new Audio(t),this._ringtoneAudio.loop=!0,this._ringtoneAudio.play().catch(()=>{})}catch{}}_stopRingtone(){this._ringtoneAudio&&(this._ringtoneAudio.pause(),this._ringtoneAudio.currentTime=0,this._ringtoneAudio=null)}_destroyAudioElements(){this._stopRingtone(),this._remoteAudio&&(this._remoteAudio.pause(),this._remoteAudio.srcObject=null,this._remoteAudio.remove(),this._remoteAudio=null)}};Bt.styles=[_t,vt,mt,ft,yt,Ct,n`
      .card-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 16px 16px 8px;
      }

      .card-title {
        font-size: 18px;
        font-weight: 500;
        margin: 0;
      }

      .card-header__actions {
        display: flex;
        gap: 4px;
      }

      .extension-info {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 8px 16px 16px;
      }

      .extension-avatar {
        width: 44px;
        height: 44px;
        border-radius: 50%;
        background-color: var(--voip-primary);
        color: #fff;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 18px;
        font-weight: 500;
        flex-shrink: 0;
      }

      .extension-details {
        flex: 1;
        min-width: 0;
      }

      .extension-name {
        font-size: 16px;
        font-weight: 500;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .extension-number {
        font-size: 13px;
        color: var(--voip-secondary-text);
      }

      .extension-status {
        display: flex;
        align-items: center;
        font-size: 12px;
        color: var(--voip-secondary-text);
        margin-top: 2px;
      }

      .active-call-banner {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 10px 16px;
        background-color: rgba(67, 160, 71, 0.1);
        border-bottom: 1px solid var(--voip-divider);
        cursor: pointer;
      }

      .active-call-banner:hover {
        background-color: rgba(67, 160, 71, 0.15);
      }

      .active-call-info {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1;
        min-width: 0;
      }

      .active-call-name {
        font-size: 14px;
        font-weight: 500;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }

      .active-call-timer {
        font-size: 13px;
        font-variant-numeric: tabular-nums;
        color: var(--voip-success);
        flex-shrink: 0;
      }

      .quick-dial-section {
        padding: 12px 16px;
        border-bottom: 1px solid var(--voip-divider);
      }

      .quick-dial-title {
        font-size: 12px;
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        color: var(--voip-secondary-text);
        margin: 0 0 8px;
      }

      .quick-dial-grid {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
      }

      .quick-dial-chip {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 6px 14px;
        border-radius: 20px;
        border: 1px solid var(--voip-divider);
        background: none;
        font-size: 13px;
        font-family: inherit;
        color: var(--voip-primary-text);
        cursor: pointer;
        transition: background-color 0.15s, border-color 0.15s;
      }

      .quick-dial-chip:hover {
        background-color: rgba(0, 0, 0, 0.04);
        border-color: var(--voip-primary);
      }

      .quick-dial-chip:active {
        background-color: var(--voip-primary);
        color: #fff;
        border-color: var(--voip-primary);
      }

      .section-title {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 12px 16px 4px;
      }

      .section-title h3 {
        font-size: 12px;
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.5px;
        color: var(--voip-secondary-text);
        margin: 0;
      }

      .dialpad-section {
        border-top: 1px solid var(--voip-divider);
      }

      .empty-state {
        text-align: center;
        padding: 20px;
        color: var(--voip-disabled);
        font-size: 13px;
      }

      .nav-tabs {
        display: flex;
        border-bottom: 1px solid var(--voip-divider);
      }

      .nav-tab {
        flex: 1;
        padding: 10px;
        text-align: center;
        font-size: 13px;
        font-weight: 500;
        color: var(--voip-secondary-text);
        background: none;
        border: none;
        border-bottom: 2px solid transparent;
        cursor: pointer;
        font-family: inherit;
        transition: color 0.2s, border-color 0.2s;
      }

      .nav-tab:hover {
        color: var(--voip-primary-text);
      }

      .nav-tab--active {
        color: var(--voip-primary);
        border-bottom-color: var(--voip-primary);
      }
    `],t([pt({attribute:!1})],Bt.prototype,"hass",void 0),t([ut()],Bt.prototype,"_config",void 0),t([ut()],Bt.prototype,"_callState",void 0),t([ut()],Bt.prototype,"_extensions",void 0),t([ut()],Bt.prototype,"_history",void 0),t([ut()],Bt.prototype,"_showPopup",void 0),t([ut()],Bt.prototype,"_showDialpad",void 0),t([ut()],Bt.prototype,"_showDiagnostics",void 0),t([ut()],Bt.prototype,"_showOnboarding",void 0),t([ut()],Bt.prototype,"_view",void 0),t([ut()],Bt.prototype,"_incomingCameraEntity",void 0),Bt=t([ct("ha-voip-card")],Bt),window.customCards=window.customCards||[],window.customCards.push({type:"ha-voip-card",name:"VoIP Phone Card",description:"A full-featured VoIP calling interface for Home Assistant with WebRTC support, dialpad, call history, and doorbell camera integration.",preview:!0});export{Bt as HaVoipCard,Ut as HaVoipCardEditor};
