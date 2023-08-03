import IconWrapper from './Wrapper';

const RawIcon = (
  <svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path
      fillRule="evenodd"
      clipRule="evenodd"
      d="M12.6301 0.257265C12.6226 0.210036 12.6067 0.165535 12.5835 0.12709C12.5603 0.0886454 12.5305 0.0572526 12.4962 0.0352601C12.462 0.0132677 12.4242 0.00124506 12.3856 9.16541e-05C12.3471 -0.00106176 12.3089 0.00868392 12.2739 0.0285996C12.005 0.179266 11.7362 0.344598 11.4674 0.527931C11.2989 0.642597 11.1597 0.761263 11.0168 0.869263C10.8317 1.0086 10.6445 1.13859 10.4631 1.29126C10.3298 1.40326 10.2029 1.53193 10.0717 1.65059C9.88392 1.82126 9.69299 1.98526 9.50845 2.17059C9.39325 2.28659 9.28338 2.41859 9.16924 2.54059C8.9767 2.74525 8.78203 2.94592 8.59589 3.16592C8.49829 3.28125 8.40655 3.40992 8.31055 3.52992C8.11481 3.77258 7.91748 4.01258 7.7292 4.27191C7.6428 4.39125 7.56173 4.52258 7.47746 4.64525C7.28706 4.91925 7.09559 5.19058 6.91425 5.47991C6.82465 5.62391 6.74305 5.77791 6.65558 5.92524C6.48704 6.20857 6.31584 6.48857 6.15636 6.78524C6.0385 7.0019 5.93183 7.23324 5.81929 7.45723C5.70729 7.6799 5.58995 7.8959 5.48435 8.1239C5.25117 8.62443 5.03201 9.13496 4.82726 9.65456C4.00058 11.7565 3.53657 13.4585 3.20482 15.6265C3.17709 15.8065 3.27203 15.9852 3.41816 15.9965C4.08271 16.0479 4.29285 15.5279 4.51259 14.5312C4.56912 14.2739 4.62993 14.0192 4.69553 13.7672C4.71344 13.7003 4.74745 13.642 4.79289 13.6003C4.83834 13.5585 4.893 13.5353 4.9494 13.5339C6.69131 13.4639 8.90257 12.2905 10.2765 10.5292C10.3602 10.4219 10.2914 10.2446 10.1719 10.2479C10.1298 10.2492 10.0882 10.2492 10.0461 10.2492C9.50205 10.2492 8.9815 10.1599 8.49562 9.99856C8.41029 9.97056 8.42415 9.81989 8.51162 9.80656C8.64336 9.78656 8.77563 9.76189 8.90683 9.72989C10.0045 9.47789 10.9581 8.88923 11.6712 8.09123C11.8365 7.70123 11.9858 7.2939 12.1192 6.86924C12.1272 6.84234 12.1297 6.81332 12.1263 6.78491C12.123 6.7565 12.1138 6.72963 12.0999 6.70683C12.0859 6.68402 12.0675 6.66604 12.0464 6.65457C12.0254 6.64309 12.0023 6.63851 11.9794 6.64124C11.5799 6.68588 11.1778 6.67918 10.7794 6.62124C10.7223 6.61257 10.717 6.50924 10.7719 6.48657C11.2526 6.28191 11.7113 6.00428 12.1378 5.65991C12.3896 5.45524 12.5576 5.12658 12.6163 4.75725C12.8616 3.20925 12.8563 1.67393 12.6301 0.257265Z"
      fill="currentColor"
    />
  </svg>
);

const BoxedIcon = (
  <svg
    width="16"
    height="16"
    viewBox="0 0 16 16"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
  >
    <path
      fillRule="evenodd"
      clipRule="evenodd"
      d="M12.6301 0.257265C12.6226 0.210036 12.6067 0.165535 12.5835 0.12709C12.5603 0.0886454 12.5305 0.0572526 12.4962 0.0352601C12.462 0.0132677 12.4242 0.00124506 12.3856 9.16541e-05C12.3471 -0.00106176 12.3089 0.00868392 12.2739 0.0285996C12.005 0.179266 11.7362 0.344598 11.4674 0.527931C11.2989 0.642597 11.1597 0.761263 11.0168 0.869263C10.8317 1.0086 10.6445 1.13859 10.4631 1.29126C10.3298 1.40326 10.2029 1.53193 10.0717 1.65059C9.88393 1.82126 9.69299 1.98526 9.50845 2.17059C9.39325 2.28659 9.28338 2.41859 9.16924 2.54059C8.9767 2.74525 8.78203 2.94592 8.59589 3.16592C8.49829 3.28125 8.40655 3.40992 8.31055 3.52992C8.11481 3.77258 7.91748 4.01258 7.7292 4.27191C7.6428 4.39125 7.56173 4.52258 7.47746 4.64525C7.28706 4.91925 7.09559 5.19058 6.91425 5.47991C6.82465 5.62391 6.74305 5.77791 6.65558 5.92524C6.48704 6.20857 6.31584 6.48857 6.15636 6.78524C6.0385 7.0019 5.93183 7.23324 5.81929 7.45723C5.70729 7.6799 5.58995 7.8959 5.48435 8.1239C5.25117 8.62443 5.03201 9.13496 4.82726 9.65456C4.00058 11.7566 3.53657 13.4585 3.20482 15.6265C3.17709 15.8065 3.27203 15.9852 3.41816 15.9965C4.08271 16.0479 4.29285 15.5279 4.51259 14.5312C4.56912 14.2739 4.62993 14.0192 4.69553 13.7672C4.71344 13.7003 4.74745 13.642 4.79289 13.6003C4.83834 13.5585 4.893 13.5353 4.9494 13.5339C6.69131 13.4639 8.90257 12.2905 10.2765 10.5292C10.3602 10.4219 10.2914 10.2446 10.1719 10.2479C10.1298 10.2492 10.0882 10.2492 10.0461 10.2492C9.50205 10.2492 8.9815 10.1599 8.49562 9.99856C8.41029 9.97056 8.42415 9.81989 8.51162 9.80656C8.64336 9.78656 8.77563 9.76189 8.90683 9.72989C10.0045 9.47789 10.9581 8.88923 11.6712 8.09123C11.8365 7.70123 11.9858 7.2939 12.1192 6.86924C12.1272 6.84234 12.1297 6.81332 12.1263 6.78491C12.123 6.7565 12.1138 6.72963 12.0999 6.70683C12.0859 6.68402 12.0675 6.66604 12.0464 6.65457C12.0254 6.64309 12.0023 6.63851 11.9794 6.64124C11.5799 6.68588 11.1778 6.67918 10.7794 6.62124C10.7223 6.61257 10.717 6.50924 10.7719 6.48657C11.2526 6.28191 11.7113 6.00428 12.1378 5.65991C12.3896 5.45524 12.5576 5.12658 12.6163 4.75725C12.8616 3.20925 12.8563 1.67393 12.6301 0.257265V0.257265Z"
      fill="currentColor"
    />
  </svg>
);

export default IconWrapper(RawIcon, BoxedIcon);
