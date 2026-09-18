device:
{
  _attrs:
  {
    schemaVersion: "1.1"
    "xmlns:xs": "http://www.w3.org/2001/XMLSchema-instance"
    "xs:noNamespaceSchemaLocation": CMSIS-SVD.xsd
  }
  addressUnitBits: "8"
  description: "CH32V103xx View File"
  name: CH32V103xx
  peripherals:
  {
    peripheral: [
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40007000"
        description: "Power control"
        groupName: PWR
        interrupt:
        {
          description: "PVD through EXTI line detection
        interrupt"
          name: PVD
          value: "17"
        }
        name: PWR
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Power control register
          (PWR_CTRL)"
              displayName: CTLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Low Power Deep Sleep", name: LPDS }
                  { bitOffset: "1", bitWidth: "1", description: "Power Down Deep Sleep", name: PDDS }
                  { bitOffset: "2", bitWidth: "1", description: "Clear Wake-up Flag", name: CWUF }
                  { bitOffset: "3", bitWidth: "1", description: "Clear STANDBY Flag", name: CSBF }
                  { bitOffset: "4", bitWidth: "1", description: "Power Voltage Detector
              Enable", name: PVDE }
                  { bitOffset: "5", bitWidth: "3", description: "PVD Level Selection", name: PLS }
                  { bitOffset: "8", bitWidth: "1", description: "Disable Backup Domain write
              protection", name: DBP }
                ]
              }
              name: CTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x04"
              description: "Power control register
          (PWR_CSR)"
              displayName: CSR
              fields:
              {
                field: [
                  { access: read-only, bitOffset: "0", bitWidth: "1", description: "Wake-Up Flag", name: WUF }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "STANDBY Flag", name: SBF }
                  { access: read-only, bitOffset: "2", bitWidth: "1", description: "PVD Output", name: PVDO }
                  { access: read-write, bitOffset: "8", bitWidth: "1", description: "Enable WKUP pin", name: EWUP }
                ]
              }
              name: CSR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40021000"
        description: "Reset and clock control"
        groupName: RCC
        interrupt:
        {
          description: "RCC global interrupt"
          name: RCC
          value: "5"
        }
        name: RCC
        registers:
        {
          register: [
            {
              addressOffset: "0x00"
              description: "Clock control register"
              displayName: CTLR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "1", description: "Internal High Speed clock enable", name: HSION }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "Internal High Speed clock ready flag", name: HSIRDY }
                  { access: read-write, bitOffset: "3", bitWidth: "5", description: "Internal High Speed clock trimming", name: HSITRIM }
                  { access: read-only, bitOffset: "8", bitWidth: "8", description: "Internal High Speed clock Calibration", name: HSICAL }
                  { access: read-write, bitOffset: "16", bitWidth: "1", description: "External High Speed clock enable", name: HSEON }
                  { access: read-only, bitOffset: "17", bitWidth: "1", description: "External High Speed clock ready flag", name: HSERDY }
                  { access: read-write, bitOffset: "18", bitWidth: "1", description: "External High Speed clock Bypass", name: HSEBYP }
                  { access: read-write, bitOffset: "19", bitWidth: "1", description: "Clock Security System enable", name: CSSON }
                  { access: read-write, bitOffset: "24", bitWidth: "1", description: "PLL enable", name: PLLON }
                  { access: read-only, bitOffset: "25", bitWidth: "1", description: "PLL clock ready flag", name: PLLRDY }
                ]
              }
              name: CTLR
              resetValue: "0x00000083"
              size: "0x20"
            }
            {
              addressOffset: "0x04"
              description: "Clock configuration register(RCC_CFGR0)"
              displayName: CFGR0
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "2", description: "System clock Switch", name: SW }
                  { access: read-only, bitOffset: "2", bitWidth: "2", description: "System Clock Switch Status", name: SWS }
                  { access: read-write, bitOffset: "4", bitWidth: "4", description: "HB prescaler", name: HPRE }
                  { access: read-write, bitOffset: "8", bitWidth: "3", description: "PB Low speed prescaler(APB1)", name: PPRE1 }
                  { access: read-write, bitOffset: "11", bitWidth: "3", description: "PB High speed prescaler(APB2)", name: PPRE2 }
                  { access: read-write, bitOffset: "14", bitWidth: "2", description: "ADC prescaler", name: ADCPRE }
                  { access: read-write, bitOffset: "16", bitWidth: "1", description: "PLL entry clock source", name: PLLSRC }
                  { access: read-write, bitOffset: "17", bitWidth: "1", description: "HSE divider for PLL entry", name: PLLXTPRE }
                  { access: read-write, bitOffset: "18", bitWidth: "4", description: "PLL Multiplication Factor", name: PLLMUL }
                  { access: read-write, bitOffset: "22", bitWidth: "1", description: "USB prescaler", name: USBPRE }
                  { access: read-write, bitOffset: "24", bitWidth: "3", description: "Microcontroller clock output", name: MCO }
                ]
              }
              name: CFGR0
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x08"
              description: "Clock interrupt register(RCC_INTR)"
              displayName: INTR
              fields:
              {
                field: [
                  { access: read-only, bitOffset: "0", bitWidth: "1", description: "LSI Ready Interrupt flag", name: LSIRDYF }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "LSE Ready Interrupt flag", name: LSERDYF }
                  { access: read-only, bitOffset: "2", bitWidth: "1", description: "HSI Ready Interrupt flag", name: HSIRDYF }
                  { access: read-only, bitOffset: "3", bitWidth: "1", description: "HSE Ready Interrupt flag", name: HSERDYF }
                  { access: read-only, bitOffset: "4", bitWidth: "1", description: "PLL Ready Interrupt flag", name: PLLRDYF }
                  { access: read-only, bitOffset: "7", bitWidth: "1", description: "Clock Security System Interrupt flag", name: CSSF }
                  { access: read-write, bitOffset: "8", bitWidth: "1", description: "LSI Ready Interrupt Enable", name: LSIRDYIE }
                  { access: read-write, bitOffset: "9", bitWidth: "1", description: "LSE Ready Interrupt Enable", name: LSERDYIE }
                  { access: read-write, bitOffset: "10", bitWidth: "1", description: "HSI Ready Interrupt Enable", name: HSIRDYIE }
                  { access: read-write, bitOffset: "11", bitWidth: "1", description: "HSE Ready Interrupt Enable", name: HSERDYIE }
                  { access: read-write, bitOffset: "12", bitWidth: "1", description: "PLL Ready Interrupt Enable", name: PLLRDYIE }
                  { access: write-only, bitOffset: "16", bitWidth: "1", description: "LSI Ready Interrupt Clear", name: LSIRDYC }
                  { access: write-only, bitOffset: "17", bitWidth: "1", description: "LSE Ready Interrupt Clear", name: LSERDYC }
                  { access: write-only, bitOffset: "18", bitWidth: "1", description: "HSI Ready Interrupt Clear", name: HSIRDYC }
                  { access: write-only, bitOffset: "19", bitWidth: "1", description: "HSE Ready Interrupt Clear", name: HSERDYC }
                  { access: write-only, bitOffset: "20", bitWidth: "1", description: "PLL Ready Interrupt Clear", name: PLLRDYC }
                  { access: write-only, bitOffset: "23", bitWidth: "1", description: "Clock security system interrupt clear", name: CSSC }
                ]
              }
              name: INTR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x0C"
              description: "PB2 peripheral reset register(RCC_APB2PRSTR)"
              displayName: APB2PRSTR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Alternate function I/O
              reset", name: AFIORST }
                  { bitOffset: "2", bitWidth: "1", description: "IO port A reset", name: IOPARST }
                  { bitOffset: "3", bitWidth: "1", description: "IO port B reset", name: IOPBRST }
                  { bitOffset: "4", bitWidth: "1", description: "IO port C reset", name: IOPCRST }
                  { bitOffset: "5", bitWidth: "1", description: "IO port D reset", name: IOPDRST }
                  { bitOffset: "9", bitWidth: "1", description: "ADC interface reset", name: ADCRST }
                  { bitOffset: "11", bitWidth: "1", description: "TIM1 timer reset", name: TIM1RST }
                  { bitOffset: "12", bitWidth: "1", description: "SPI 1 reset", name: SPI1RST }
                  { bitOffset: "14", bitWidth: "1", description: "USART1 reset", name: USART1RST }
                ]
              }
              name: APB2PRSTR
              resetValue: "0x000000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "PB1 peripheral reset register(RCC_APB1PRSTR)"
              displayName: APB1PRSTR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Timer 2 reset", name: TIM2RST }
                  { bitOffset: "1", bitWidth: "1", description: "Timer 3 reset", name: TIM3RST }
                  { bitOffset: "2", bitWidth: "1", description: "Timer 4 reset", name: TIM4RST }
                  { bitOffset: "11", bitWidth: "1", description: "Window watchdog reset", name: WWDGRST }
                  { bitOffset: "14", bitWidth: "1", description: "SPI2 reset", name: SPI2RST }
                  { bitOffset: "17", bitWidth: "1", description: "USART 2 reset", name: USART2RST }
                  { bitOffset: "18", bitWidth: "1", description: "USART 3 reset", name: USART3RST }
                  { bitOffset: "21", bitWidth: "1", description: "I2C1 reset", name: I2C1RST }
                  { bitOffset: "22", bitWidth: "1", description: "I2C2 reset", name: I2C2RST }
                  { bitOffset: "23", bitWidth: "1", description: "USBD reset", name: USBDRST }
                  { bitOffset: "25", bitWidth: "1", description: "CAN reset", name: CANRST }
                  { bitOffset: "27", bitWidth: "1", description: "Backup interface reset", name: BKPRST }
                  { bitOffset: "28", bitWidth: "1", description: "Power interface reset", name: PWRRST }
                  { bitOffset: "29", bitWidth: "1", description: "DAC interface reset", name: DACRST }
                ]
              }
              name: APB1PRSTR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "HB Peripheral Clock enable register(RCC_AHBPCENR)"
              displayName: AHBPCENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "DMA clock enable", name: DMAEN }
                  { bitOffset: "2", bitWidth: "1", description: "SRAM interface clock
              enable", name: SRAMEN }
                  { bitOffset: "4", bitWidth: "1", description: "FLITF clock enable", name: FLITFEN }
                  { bitOffset: "6", bitWidth: "1", description: "CRC clock enable", name: CRCEN }
                  { bitOffset: "12", bitWidth: "1", description: "USBHD clock enable", name: USBHDEN }
                ]
              }
              name: AHBPCENR
              resetValue: "0x00000014"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "PB2 peripheral clock enable register
          (RCC_APB2PCENR)"
              displayName: APB2PCENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Alternate function I/O clock
              enable", name: AFIOEN }
                  { bitOffset: "2", bitWidth: "1", description: "I/O port A clock enable", name: IOPAEN }
                  { bitOffset: "3", bitWidth: "1", description: "I/O port B clock enable", name: IOPBEN }
                  { bitOffset: "4", bitWidth: "1", description: "I/O port C clock enable", name: IOPCEN }
                  { bitOffset: "5", bitWidth: "1", description: "I/O port D clock enable", name: IOPDEN }
                  { bitOffset: "9", bitWidth: "1", description: "ADC interface clock
              enable", name: ADCEN }
                  { bitOffset: "11", bitWidth: "1", description: "TIM1 Timer clock enable", name: TIM1EN }
                  { bitOffset: "12", bitWidth: "1", description: "SPI 1 clock enable", name: SPI1EN }
                  { bitOffset: "14", bitWidth: "1", description: "USART1 clock enable", name: USART1EN }
                ]
              }
              name: APB2PCENR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "PB1 peripheral clock enable register
          (RCC_APB1PCENR)"
              displayName: APB1PCENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Timer 2 clock enable", name: TIM2EN }
                  { bitOffset: "1", bitWidth: "1", description: "Timer 3 clock enable", name: TIM3EN }
                  { bitOffset: "2", bitWidth: "1", description: "Timer 4 clock enable", name: TIM4EN }
                  { bitOffset: "11", bitWidth: "1", description: "Window watchdog clock
              enable", name: WWDGEN }
                  { bitOffset: "14", bitWidth: "1", description: "SPI 2 clock enable", name: SPI2EN }
                  { bitOffset: "17", bitWidth: "1", description: "USART 2 clock enable", name: USART2EN }
                  { bitOffset: "18", bitWidth: "1", description: "USART 3 clock enable", name: USART3EN }
                  { bitOffset: "21", bitWidth: "1", description: "I2C 1 clock enable", name: I2C1EN }
                  { bitOffset: "22", bitWidth: "1", description: "I2C 2 clock enable", name: I2C2EN }
                  { bitOffset: "23", bitWidth: "1", description: "USBD clock enable", name: USBDEN }
                  { bitOffset: "25", bitWidth: "1", description: "CAN clock enable", name: CANEN }
                  { bitOffset: "27", bitWidth: "1", description: "Backup interface clock
              enable", name: BKPEN }
                  { bitOffset: "28", bitWidth: "1", description: "Power interface clock
              enable", name: PWREN }
                  { bitOffset: "29", bitWidth: "1", description: "DAC interface clock enable", name: DACEN }
                ]
              }
              name: APB1PCENR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x20"
              description: "Backup domain control register(RCC_BDCTLR)"
              displayName: BDCTLR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "1", description: "External Low Speed oscillator enable", name: LSEON }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "External Low Speed oscillator ready", name: LSERDY }
                  { access: read-write, bitOffset: "2", bitWidth: "1", description: "External Low Speed oscillator bypass", name: LSEBYP }
                  { access: read-write, bitOffset: "8", bitWidth: "2", description: "RTC clock source selection", name: RTCSEL }
                  { access: read-write, bitOffset: "15", bitWidth: "1", description: "RTC clock enable", name: RTCEN }
                  { access: read-write, bitOffset: "16", bitWidth: "1", description: "Backup domain software reset", name: BDRST }
                ]
              }
              name: BDCTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x24"
              description: "Control/status register(RCC_RSTSCKR)"
              displayName: RSTSCKR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "1", description: "Internal low speed oscillator enable", name: LSION }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "Internal low speed oscillator ready", name: LSIRDY }
                  { access: read-write, bitOffset: "24", bitWidth: "1", description: "Remove reset flag", name: RMVF }
                  { access: read-write, bitOffset: "26", bitWidth: "1", description: "PIN reset flag", name: PINRSTF }
                  { access: read-write, bitOffset: "27", bitWidth: "1", description: "POR/PDR reset flag", name: PORRSTF }
                  { access: read-write, bitOffset: "28", bitWidth: "1", description: "Software reset flag", name: SFTRSTF }
                  { access: read-write, bitOffset: "29", bitWidth: "1", description: "Independent watchdog reset
              flag", name: IWDGRSTF }
                  { access: read-write, bitOffset: "30", bitWidth: "1", description: "Window watchdog reset flag", name: WWDGRSTF }
                  { access: read-write, bitOffset: "31", bitWidth: "1", description: "Low-power reset flag", name: LPWRRSTF }
                ]
              }
              name: RSTSCKR
              resetValue: "0x0C000000"
              size: "0x20"
            }
            {
              addressOffset: "0x28"
              description: "HB reset register(RCC_APHBRSTR)"
              displayName: AHBRSTR
              fields:
              {
                field:
                {
                  access: read-write
                  bitOffset: "12"
                  bitWidth: "1"
                  description: "USBHD reset"
                  name: USBHDRST
                }
              }
              name: AHBRSTR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x800"
          usage: registers
        }
        baseAddress: "0x40023800"
        description: "extension configuration"
        groupName: EXTEND
        name: EXTEND
        registers:
        {
          register:
          {
            addressOffset: "0X00"
            description: "EXTEND register"
            displayName: EXTEND_CTR
            fields:
            {
              field: [
                { access: read-write, bitOffset: "0", bitWidth: "1", description: "USBD Lowspeed Enable", name: USBDLS }
                { access: read-write, bitOffset: "1", bitWidth: "1", description: "USBD pullup Enable", name: USBDPU }
                { access: read-write, bitOffset: "2", bitWidth: "1", description: "USBHD IO(PB6/PB7) Enable", name: USBHDIO }
                { access: read-write, bitOffset: "3", bitWidth: "1", description: "USB 5V Enable", name: USB5VSEL }
                { access: read-write, bitOffset: "4", bitWidth: "1", description: "Whether HSI is divided", name: HSIPRE }
                { access: read-write, bitOffset: "6", bitWidth: "1", description: LOCKUP, name: LKUPEN }
                { access: read-write, bitOffset: "7", bitWidth: "1", description: "LOCKUP RESET", name: LKUPRST }
                { access: read-write, bitOffset: "8", bitWidth: "2", description: ULLDOTRIM, name: ULLDOTRIM }
                { access: read-write, bitOffset: "10", bitWidth: "1", description: LDOTRIM, name: LDOTRIM }
              ]
            }
            name: EXTEND_CTR
            resetValue: "0x00000200"
            size: "0x20"
          }
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40010800"
        description: "General purpose I/O"
        groupName: GPIO
        name: GPIOA
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x00"
              description: "Port configuration register low(GPIOn_CFGLR)"
              displayName: CFGLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "2", description: "Port n.0 mode bits", name: MODE0 }
                  { bitOffset: "2", bitWidth: "2", description: "Port n.0 configuration bits", name: CNF0 }
                  { bitOffset: "4", bitWidth: "2", description: "Port n.1 mode bits", name: MODE1 }
                  { bitOffset: "6", bitWidth: "2", description: "Port n.1 configuration bits", name: CNF1 }
                  { bitOffset: "8", bitWidth: "2", description: "Port n.2 mode bits", name: MODE2 }
                  { bitOffset: "10", bitWidth: "2", description: "Port n.2 configuration bits", name: CNF2 }
                  { bitOffset: "12", bitWidth: "2", description: "Port n.3 mode bits", name: MODE3 }
                  { bitOffset: "14", bitWidth: "2", description: "Port n.3 configuration bits", name: CNF3 }
                  { bitOffset: "16", bitWidth: "2", description: "Port n.4 mode bits", name: MODE4 }
                  { bitOffset: "18", bitWidth: "2", description: "Port n.4 configuration bits", name: CNF4 }
                  { bitOffset: "20", bitWidth: "2", description: "Port n.5 mode bits", name: MODE5 }
                  { bitOffset: "22", bitWidth: "2", description: "Port n.5 configuration bits", name: CNF5 }
                  { bitOffset: "24", bitWidth: "2", description: "Port n.6 mode bits", name: MODE6 }
                  { bitOffset: "26", bitWidth: "2", description: "Port n.6 configuration bits", name: CNF6 }
                  { bitOffset: "28", bitWidth: "2", description: "Port n.7 mode bits", name: MODE7 }
                  { bitOffset: "30", bitWidth: "2", description: "Port n.7 configuration bits", name: CNF7 }
                ]
              }
              name: CFGLR
              resetValue: "0x44444444"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x04"
              description: "Port configuration register high
          (GPIOn_CFGHR)"
              displayName: CFGHR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "2", description: "Port n.8 mode bits", name: MODE8 }
                  { bitOffset: "2", bitWidth: "2", description: "Port n.8 configuration
              bits", name: CNF8 }
                  { bitOffset: "4", bitWidth: "2", description: "Port n.9 mode bits", name: MODE9 }
                  { bitOffset: "6", bitWidth: "2", description: "Port n.9 configuration
              bits", name: CNF9 }
                  { bitOffset: "8", bitWidth: "2", description: "Port n.10 mode bits", name: MODE10 }
                  { bitOffset: "10", bitWidth: "2", description: "Port n.10 configuration
              bits", name: CNF10 }
                  { bitOffset: "12", bitWidth: "2", description: "Port n.11 mode bits", name: MODE11 }
                  { bitOffset: "14", bitWidth: "2", description: "Port n.11 configuration
              bits", name: CNF11 }
                  { bitOffset: "16", bitWidth: "2", description: "Port n.12 mode bits", name: MODE12 }
                  { bitOffset: "18", bitWidth: "2", description: "Port n.12 configuration
              bits", name: CNF12 }
                  { bitOffset: "20", bitWidth: "2", description: "Port n.13 mode bits", name: MODE13 }
                  { bitOffset: "22", bitWidth: "2", description: "Port n.13 configuration
              bits", name: CNF13 }
                  { bitOffset: "24", bitWidth: "2", description: "Port n.14 mode bits", name: MODE14 }
                  { bitOffset: "26", bitWidth: "2", description: "Port n.14 configuration
              bits", name: CNF14 }
                  { bitOffset: "28", bitWidth: "2", description: "Port n.15 mode bits", name: MODE15 }
                  { bitOffset: "30", bitWidth: "2", description: "Port n.15 configuration
              bits", name: CNF15 }
                ]
              }
              name: CFGHR
              resetValue: "0x44444444"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x08"
              description: "Port input data register
          (GPIOn_INDR)"
              displayName: INDR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Port input data", name: IDR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Port input data", name: IDR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Port input data", name: IDR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Port input data", name: IDR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Port input data", name: IDR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Port input data", name: IDR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Port input data", name: IDR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Port input data", name: IDR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Port input data", name: IDR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Port input data", name: IDR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Port input data", name: IDR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Port input data", name: IDR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Port input data", name: IDR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Port input data", name: IDR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Port input data", name: IDR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Port input data", name: IDR15 }
                ]
              }
              name: INDR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x0C"
              description: "Port output data register
          (GPIOn_OUTDR)"
              displayName: OUTDR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Port output data", name: ODR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Port output data", name: ODR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Port output data", name: ODR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Port output data", name: ODR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Port output data", name: ODR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Port output data", name: ODR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Port output data", name: ODR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Port output data", name: ODR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Port output data", name: ODR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Port output data", name: ODR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Port output data", name: ODR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Port output data", name: ODR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Port output data", name: ODR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Port output data", name: ODR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Port output data", name: ODR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Port output data", name: ODR15 }
                ]
              }
              name: OUTDR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x10"
              description: "Port bit set/reset register
          (GPIOn_BSHR)"
              displayName: BSHR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Set bit 0", name: BS0 }
                  { bitOffset: "1", bitWidth: "1", description: "Set bit 1", name: BS1 }
                  { bitOffset: "2", bitWidth: "1", description: "Set bit 1", name: BS2 }
                  { bitOffset: "3", bitWidth: "1", description: "Set bit 3", name: BS3 }
                  { bitOffset: "4", bitWidth: "1", description: "Set bit 4", name: BS4 }
                  { bitOffset: "5", bitWidth: "1", description: "Set bit 5", name: BS5 }
                  { bitOffset: "6", bitWidth: "1", description: "Set bit 6", name: BS6 }
                  { bitOffset: "7", bitWidth: "1", description: "Set bit 7", name: BS7 }
                  { bitOffset: "8", bitWidth: "1", description: "Set bit 8", name: BS8 }
                  { bitOffset: "9", bitWidth: "1", description: "Set bit 9", name: BS9 }
                  { bitOffset: "10", bitWidth: "1", description: "Set bit 10", name: BS10 }
                  { bitOffset: "11", bitWidth: "1", description: "Set bit 11", name: BS11 }
                  { bitOffset: "12", bitWidth: "1", description: "Set bit 12", name: BS12 }
                  { bitOffset: "13", bitWidth: "1", description: "Set bit 13", name: BS13 }
                  { bitOffset: "14", bitWidth: "1", description: "Set bit 14", name: BS14 }
                  { bitOffset: "15", bitWidth: "1", description: "Set bit 15", name: BS15 }
                  { bitOffset: "16", bitWidth: "1", description: "Reset bit 0", name: BR0 }
                  { bitOffset: "17", bitWidth: "1", description: "Reset bit 1", name: BR1 }
                  { bitOffset: "18", bitWidth: "1", description: "Reset bit 2", name: BR2 }
                  { bitOffset: "19", bitWidth: "1", description: "Reset bit 3", name: BR3 }
                  { bitOffset: "20", bitWidth: "1", description: "Reset bit 4", name: BR4 }
                  { bitOffset: "21", bitWidth: "1", description: "Reset bit 5", name: BR5 }
                  { bitOffset: "22", bitWidth: "1", description: "Reset bit 6", name: BR6 }
                  { bitOffset: "23", bitWidth: "1", description: "Reset bit 7", name: BR7 }
                  { bitOffset: "24", bitWidth: "1", description: "Reset bit 8", name: BR8 }
                  { bitOffset: "25", bitWidth: "1", description: "Reset bit 9", name: BR9 }
                  { bitOffset: "26", bitWidth: "1", description: "Reset bit 10", name: BR10 }
                  { bitOffset: "27", bitWidth: "1", description: "Reset bit 11", name: BR11 }
                  { bitOffset: "28", bitWidth: "1", description: "Reset bit 12", name: BR12 }
                  { bitOffset: "29", bitWidth: "1", description: "Reset bit 13", name: BR13 }
                  { bitOffset: "30", bitWidth: "1", description: "Reset bit 14", name: BR14 }
                  { bitOffset: "31", bitWidth: "1", description: "Reset bit 15", name: BR15 }
                ]
              }
              name: BSHR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x14"
              description: "Port bit reset register
          (GPIOn_BCR)"
              displayName: BCR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Reset bit 0", name: BR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Reset bit 1", name: BR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Reset bit 1", name: BR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Reset bit 3", name: BR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Reset bit 4", name: BR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Reset bit 5", name: BR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Reset bit 6", name: BR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Reset bit 7", name: BR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Reset bit 8", name: BR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Reset bit 9", name: BR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Reset bit 10", name: BR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Reset bit 11", name: BR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Reset bit 12", name: BR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Reset bit 13", name: BR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Reset bit 14", name: BR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Reset bit 15", name: BR15 }
                ]
              }
              name: BCR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "Port configuration lock
          register"
              displayName: LCKR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Port A Lock bit 0", name: LCK0 }
                  { bitOffset: "1", bitWidth: "1", description: "Port A Lock bit 1", name: LCK1 }
                  { bitOffset: "2", bitWidth: "1", description: "Port A Lock bit 2", name: LCK2 }
                  { bitOffset: "3", bitWidth: "1", description: "Port A Lock bit 3", name: LCK3 }
                  { bitOffset: "4", bitWidth: "1", description: "Port A Lock bit 4", name: LCK4 }
                  { bitOffset: "5", bitWidth: "1", description: "Port A Lock bit 5", name: LCK5 }
                  { bitOffset: "6", bitWidth: "1", description: "Port A Lock bit 6", name: LCK6 }
                  { bitOffset: "7", bitWidth: "1", description: "Port A Lock bit 7", name: LCK7 }
                  { bitOffset: "8", bitWidth: "1", description: "Port A Lock bit 8", name: LCK8 }
                  { bitOffset: "9", bitWidth: "1", description: "Port A Lock bit 9", name: LCK9 }
                  { bitOffset: "10", bitWidth: "1", description: "Port A Lock bit 10", name: LCK10 }
                  { bitOffset: "11", bitWidth: "1", description: "Port A Lock bit 11", name: LCK11 }
                  { bitOffset: "12", bitWidth: "1", description: "Port A Lock bit 12", name: LCK12 }
                  { bitOffset: "13", bitWidth: "1", description: "Port A Lock bit 13", name: LCK13 }
                  { bitOffset: "14", bitWidth: "1", description: "Port A Lock bit 14", name: LCK14 }
                  { bitOffset: "15", bitWidth: "1", description: "Port A Lock bit 15", name: LCK15 }
                  { bitOffset: "16", bitWidth: "1", description: "Lock key", name: LCKK }
                ]
              }
              name: LCKR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        _attrs:
        {
          derivedFrom: GPIOA
        }
        baseAddress: "0x40010C00"
        name: GPIOB
      }
      {
        _attrs:
        {
          derivedFrom: GPIOA
        }
        baseAddress: "0x40011000"
        name: GPIOC
      }
      {
        _attrs:
        {
          derivedFrom: GPIOA
        }
        baseAddress: "0x40011400"
        name: GPIOD
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40010000"
        description: "Alternate function I/O"
        groupName: AFIO
        name: AFIO
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Event Control Register
          (AFIO_ECR)"
              displayName: ECR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Pin selection", name: PIN }
                  { bitOffset: "4", bitWidth: "3", description: "Port selection", name: PORT }
                  { bitOffset: "7", bitWidth: "1", description: "Event Output Enable", name: EVOE }
                ]
              }
              name: ECR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x4"
              description: "AF remap and debug I/O configuration
          register (AFIO_PCFR)"
              displayName: PCFR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "1", description: "SPI1 remapping", name: SPI1_RM }
                  { access: read-write, bitOffset: "1", bitWidth: "1", description: "I2C1 remapping", name: I2C1_RM }
                  { access: read-write, bitOffset: "2", bitWidth: "1", description: "USART1 remapping", name: USART1_RM }
                  { access: read-write, bitOffset: "3", bitWidth: "1", description: "USART2 remapping", name: USART2_RM }
                  { access: read-write, bitOffset: "4", bitWidth: "2", description: "USART3 remapping", name: USART3_RM }
                  { access: read-write, bitOffset: "6", bitWidth: "2", description: "TIM1 remapping", name: TIM1_RM }
                  { access: read-write, bitOffset: "8", bitWidth: "2", description: "TIM2 remapping", name: TIM2_RM }
                  { access: read-write, bitOffset: "10", bitWidth: "2", description: "TIM3 remapping", name: TIM3_RM }
                  { access: read-write, bitOffset: "13", bitWidth: "2", description: "CAN1 remapping", name: CAN_RM }
                  { access: read-write, bitOffset: "15", bitWidth: "1", description: "Port D0/Port D1 mapping on
              OSCIN/OSCOUT", name: PD01_RM }
                  { access: read-write, bitOffset: "24", bitWidth: "3", description: "Serial wire JTAG
              configuration", name: SWCFG }
                ]
              }
              name: PCFR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "External interrupt configuration register 1
          (AFIO_EXTICR1)"
              displayName: EXTICR1
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "EXTI0 configuration", name: EXTI0 }
                  { bitOffset: "4", bitWidth: "4", description: "EXTI1 configuration", name: EXTI1 }
                  { bitOffset: "8", bitWidth: "4", description: "EXTI2 configuration", name: EXTI2 }
                  { bitOffset: "12", bitWidth: "4", description: "EXTI3 configuration", name: EXTI3 }
                ]
              }
              name: EXTICR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "External interrupt configuration register 2
          (AFIO_EXTICR2)"
              displayName: EXTICR2
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "EXTI4 configuration", name: EXTI4 }
                  { bitOffset: "4", bitWidth: "4", description: "EXTI5 configuration", name: EXTI5 }
                  { bitOffset: "8", bitWidth: "4", description: "EXTI6 configuration", name: EXTI6 }
                  { bitOffset: "12", bitWidth: "4", description: "EXTI7 configuration", name: EXTI7 }
                ]
              }
              name: EXTICR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "External interrupt configuration register 3
          (AFIO_EXTICR3)"
              displayName: EXTICR3
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "EXTI8 configuration", name: EXTI8 }
                  { bitOffset: "4", bitWidth: "4", description: "EXTI9 configuration", name: EXTI9 }
                  { bitOffset: "8", bitWidth: "4", description: "EXTI10 configuration", name: EXTI10 }
                  { bitOffset: "12", bitWidth: "4", description: "EXTI11 configuration", name: EXTI11 }
                ]
              }
              name: EXTICR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "External interrupt configuration register 4
          (AFIO_EXTICR4)"
              displayName: EXTICR4
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "EXTI12 configuration", name: EXTI12 }
                  { bitOffset: "4", bitWidth: "4", description: "EXTI13 configuration", name: EXTI13 }
                  { bitOffset: "8", bitWidth: "4", description: "EXTI14 configuration", name: EXTI14 }
                  { bitOffset: "12", bitWidth: "4", description: "EXTI15 configuration", name: EXTI15 }
                ]
              }
              name: EXTICR4
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40010400"
        description: EXTI
        groupName: EXTI
        interrupt: [
          { description: "Tamper interrupt", name: TAMPER, value: "18" }
          { description: "EXTI Line0 interrupt", name: EXTI0, value: "22" }
          { description: "EXTI Line1 interrupt", name: EXTI1, value: "23" }
          { description: "EXTI Line2 interrupt", name: EXTI2, value: "24" }
          { description: "EXTI Line3 interrupt", name: EXTI3, value: "25" }
          { description: "EXTI Line4 interrupt", name: EXTI4, value: "26" }
          { description: "EXTI Line[9:5] interrupts", name: EXTI9_5, value: "39" }
          { description: "EXTI Line[15:10] interrupts", name: EXTI15_10, value: "56" }
        ]
        name: EXTI
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x00"
              description: "Interrupt mask register(EXTI_INTENR)"
              displayName: INTENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Interrupt Mask on line 0", name: MR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Interrupt Mask on line 1", name: MR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Interrupt Mask on line 2", name: MR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Interrupt Mask on line 3", name: MR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Interrupt Mask on line 4", name: MR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Interrupt Mask on line 5", name: MR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Interrupt Mask on line 6", name: MR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Interrupt Mask on line 7", name: MR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Interrupt Mask on line 8", name: MR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Interrupt Mask on line 9", name: MR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Interrupt Mask on line 10", name: MR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Interrupt Mask on line 11", name: MR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Interrupt Mask on line 12", name: MR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Interrupt Mask on line 13", name: MR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Interrupt Mask on line 14", name: MR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Interrupt Mask on line 15", name: MR15 }
                  { bitOffset: "16", bitWidth: "1", description: "Interrupt Mask on line 16", name: MR16 }
                  { bitOffset: "17", bitWidth: "1", description: "Interrupt Mask on line 17", name: MR17 }
                  { bitOffset: "18", bitWidth: "1", description: "Interrupt Mask on line 18", name: MR18 }
                ]
              }
              name: INTENR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x04"
              description: "Event mask register (EXTI_EVENR)"
              displayName: EVENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Event Mask on line 0", name: MR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Event Mask on line 1", name: MR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Event Mask on line 2", name: MR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Event Mask on line 3", name: MR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Event Mask on line 4", name: MR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Event Mask on line 5", name: MR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Event Mask on line 6", name: MR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Event Mask on line 7", name: MR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Event Mask on line 8", name: MR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Event Mask on line 9", name: MR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Event Mask on line 10", name: MR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Event Mask on line 11", name: MR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Event Mask on line 12", name: MR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Event Mask on line 13", name: MR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Event Mask on line 14", name: MR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Event Mask on line 15", name: MR15 }
                  { bitOffset: "16", bitWidth: "1", description: "Event Mask on line 16", name: MR16 }
                  { bitOffset: "17", bitWidth: "1", description: "Event Mask on line 17", name: MR17 }
                  { bitOffset: "18", bitWidth: "1", description: "Event Mask on line 18", name: MR18 }
                ]
              }
              name: EVENR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x08"
              description: "Rising Trigger selection register(EXTI_RTENR)"
              displayName: RTENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Rising trigger event configuration of line 0", name: TR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Rising trigger event configuration of line 1", name: TR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Rising trigger event configuration of line 2", name: TR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Rising trigger event configuration of line 3", name: TR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Rising trigger event configuration of line 4", name: TR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Rising trigger event configuration of line 5", name: TR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Rising trigger event configuration of line 6", name: TR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Rising trigger event configuration of line 7", name: TR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Rising trigger event configuration of line 8", name: TR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Rising trigger event configuration of line 9", name: TR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Rising trigger event configuration of line 10", name: TR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Rising trigger event configuration of line 11", name: TR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Rising trigger event configuration of line 12", name: TR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Rising trigger event configuration of line 13", name: TR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Rising trigger event configuration of line 14", name: TR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Rising trigger event configuration of line 15", name: TR15 }
                  { bitOffset: "16", bitWidth: "1", description: "Rising trigger event configuration of line 16", name: TR16 }
                  { bitOffset: "17", bitWidth: "1", description: "Rising trigger event configuration of line 17", name: TR17 }
                  { bitOffset: "18", bitWidth: "1", description: "Rising trigger event configuration of line 18", name: TR18 }
                ]
              }
              name: RTENR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x0C"
              description: "Falling Trigger selection register(EXTI_FTENR)"
              displayName: FTENR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Falling trigger event configuration of line 0", name: TR0 }
                  { bitOffset: "1", bitWidth: "1", description: "Falling trigger event configuration of line 1", name: TR1 }
                  { bitOffset: "2", bitWidth: "1", description: "Falling trigger event configuration of line 2", name: TR2 }
                  { bitOffset: "3", bitWidth: "1", description: "Falling trigger event configuration of line 3", name: TR3 }
                  { bitOffset: "4", bitWidth: "1", description: "Falling trigger event configuration of line 4", name: TR4 }
                  { bitOffset: "5", bitWidth: "1", description: "Falling trigger event configuration of line 5", name: TR5 }
                  { bitOffset: "6", bitWidth: "1", description: "Falling trigger event configuration of line 6", name: TR6 }
                  { bitOffset: "7", bitWidth: "1", description: "Falling trigger event configuration of line 7", name: TR7 }
                  { bitOffset: "8", bitWidth: "1", description: "Falling trigger event configuration of line 8", name: TR8 }
                  { bitOffset: "9", bitWidth: "1", description: "Falling trigger event configuration of line 9", name: TR9 }
                  { bitOffset: "10", bitWidth: "1", description: "Falling trigger event configuration of line 10", name: TR10 }
                  { bitOffset: "11", bitWidth: "1", description: "Falling trigger event configuration of line 11", name: TR11 }
                  { bitOffset: "12", bitWidth: "1", description: "Falling trigger event configuration of line 12", name: TR12 }
                  { bitOffset: "13", bitWidth: "1", description: "Falling trigger event configuration of line 13", name: TR13 }
                  { bitOffset: "14", bitWidth: "1", description: "Falling trigger event configuration of line 14", name: TR14 }
                  { bitOffset: "15", bitWidth: "1", description: "Falling trigger event configuration of line 15", name: TR15 }
                  { bitOffset: "16", bitWidth: "1", description: "Falling trigger event configuration of line 16", name: TR16 }
                  { bitOffset: "17", bitWidth: "1", description: "Falling trigger event configuration of line 17", name: TR17 }
                  { bitOffset: "18", bitWidth: "1", description: "Falling trigger event configuration of line 18", name: TR18 }
                ]
              }
              name: FTENR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "Software interrupt event register(EXTI_SWIEVR)"
              displayName: SWIEVR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Software Interrupt on line 0", name: SWIER0 }
                  { bitOffset: "1", bitWidth: "1", description: "Software Interrupt on line 1", name: SWIER1 }
                  { bitOffset: "2", bitWidth: "1", description: "Software Interrupt on line 2", name: SWIER2 }
                  { bitOffset: "3", bitWidth: "1", description: "Software Interrupt on line 3", name: SWIER3 }
                  { bitOffset: "4", bitWidth: "1", description: "Software Interrupt on line 4", name: SWIER4 }
                  { bitOffset: "5", bitWidth: "1", description: "Software Interrupt on line 5", name: SWIER5 }
                  { bitOffset: "6", bitWidth: "1", description: "Software Interrupt on line 6", name: SWIER6 }
                  { bitOffset: "7", bitWidth: "1", description: "Software Interrupt on line 7", name: SWIER7 }
                  { bitOffset: "8", bitWidth: "1", description: "Software Interrupt on line 8", name: SWIER8 }
                  { bitOffset: "9", bitWidth: "1", description: "Software Interrupt on line 9", name: SWIER9 }
                  { bitOffset: "10", bitWidth: "1", description: "Software Interrupt on line 10", name: SWIER10 }
                  { bitOffset: "11", bitWidth: "1", description: "Software Interrupt on line 11", name: SWIER11 }
                  { bitOffset: "12", bitWidth: "1", description: "Software Interrupt on line 12", name: SWIER12 }
                  { bitOffset: "13", bitWidth: "1", description: "Software Interrupt on line 13", name: SWIER13 }
                  { bitOffset: "14", bitWidth: "1", description: "Software Interrupt on line 14", name: SWIER14 }
                  { bitOffset: "15", bitWidth: "1", description: "Software Interrupt on line 15", name: SWIER15 }
                  { bitOffset: "16", bitWidth: "1", description: "Software Interrupt on line 16", name: SWIER16 }
                  { bitOffset: "17", bitWidth: "1", description: "Software Interrupt on line 17", name: SWIER17 }
                  { bitOffset: "18", bitWidth: "1", description: "Software Interrupt on line 18", name: SWIER18 }
                ]
              }
              name: SWIEVR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "Pending register (EXTI_INTFR)"
              displayName: INTFR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Pending bit 0", name: IF0 }
                  { bitOffset: "1", bitWidth: "1", description: "Pending bit 1", name: IF1 }
                  { bitOffset: "2", bitWidth: "1", description: "Pending bit 2", name: IF2 }
                  { bitOffset: "3", bitWidth: "1", description: "Pending bit 3", name: IF3 }
                  { bitOffset: "4", bitWidth: "1", description: "Pending bit 4", name: IF4 }
                  { bitOffset: "5", bitWidth: "1", description: "Pending bit 5", name: IF5 }
                  { bitOffset: "6", bitWidth: "1", description: "Pending bit 6", name: IF6 }
                  { bitOffset: "7", bitWidth: "1", description: "Pending bit 7", name: IF7 }
                  { bitOffset: "8", bitWidth: "1", description: "Pending bit 8", name: IF8 }
                  { bitOffset: "9", bitWidth: "1", description: "Pending bit 9", name: IF9 }
                  { bitOffset: "10", bitWidth: "1", description: "Pending bit 10", name: IF10 }
                  { bitOffset: "11", bitWidth: "1", description: "Pending bit 11", name: IF11 }
                  { bitOffset: "12", bitWidth: "1", description: "Pending bit 12", name: IF12 }
                  { bitOffset: "13", bitWidth: "1", description: "Pending bit 13", name: IF13 }
                  { bitOffset: "14", bitWidth: "1", description: "Pending bit 14", name: IF14 }
                  { bitOffset: "15", bitWidth: "1", description: "Pending bit 15", name: IF15 }
                  { bitOffset: "16", bitWidth: "1", description: "Pending bit 16", name: IF16 }
                  { bitOffset: "17", bitWidth: "1", description: "Pending bit 17", name: IF17 }
                  { bitOffset: "18", bitWidth: "1", description: "Pending bit 18", name: IF18 }
                ]
              }
              name: INTFR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40020000"
        description: "DMA controller"
        groupName: DMA
        interrupt: [
          { description: "DMA1 Channel1 global interrupt", name: DMA1_CH1, value: "27" }
          { description: "DMA1 Channel2 global interrupt", name: DMA1_CH2, value: "28" }
          { description: "DMA1 Channel3 global interrupt", name: DMA1_CH3, value: "29" }
          { description: "DMA1 Channel4 global interrupt", name: DMA1_CH4, value: "30" }
          { description: "DMA1 Channel5 global interrupt", name: DMA1_CH5, value: "31" }
          { description: "DMA1 Channel6 global interrupt", name: DMA1_CH6, value: "32" }
          { description: "DMA1 Channel7 global interrupt", name: DMA1_CH7, value: "33" }
        ]
        name: DMA
        registers:
        {
          register: [
            {
              access: read-only
              addressOffset: "0x0"
              description: "DMA interrupt status register
          (DMA_INTFR)"
              displayName: INTFR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel 1 Global interrupt
              flag", name: GIF1 }
                  { bitOffset: "1", bitWidth: "1", description: "Channel 1 Transfer Complete
              flag", name: TCIF1 }
                  { bitOffset: "2", bitWidth: "1", description: "Channel 1 Half Transfer Complete
              flag", name: HTIF1 }
                  { bitOffset: "3", bitWidth: "1", description: "Channel 1 Transfer Error
              flag", name: TEIF1 }
                  { bitOffset: "4", bitWidth: "1", description: "Channel 2 Global interrupt
              flag", name: GIF2 }
                  { bitOffset: "5", bitWidth: "1", description: "Channel 2 Transfer Complete
              flag", name: TCIF2 }
                  { bitOffset: "6", bitWidth: "1", description: "Channel 2 Half Transfer Complete
              flag", name: HTIF2 }
                  { bitOffset: "7", bitWidth: "1", description: "Channel 2 Transfer Error
              flag", name: TEIF2 }
                  { bitOffset: "8", bitWidth: "1", description: "Channel 3 Global interrupt
              flag", name: GIF3 }
                  { bitOffset: "9", bitWidth: "1", description: "Channel 3 Transfer Complete
              flag", name: TCIF3 }
                  { bitOffset: "10", bitWidth: "1", description: "Channel 3 Half Transfer Complete
              flag", name: HTIF3 }
                  { bitOffset: "11", bitWidth: "1", description: "Channel 3 Transfer Error
              flag", name: TEIF3 }
                  { bitOffset: "12", bitWidth: "1", description: "Channel 4 Global interrupt
              flag", name: GIF4 }
                  { bitOffset: "13", bitWidth: "1", description: "Channel 4 Transfer Complete
              flag", name: TCIF4 }
                  { bitOffset: "14", bitWidth: "1", description: "Channel 4 Half Transfer Complete
              flag", name: HTIF4 }
                  { bitOffset: "15", bitWidth: "1", description: "Channel 4 Transfer Error
              flag", name: TEIF4 }
                  { bitOffset: "16", bitWidth: "1", description: "Channel 5 Global interrupt
              flag", name: GIF5 }
                  { bitOffset: "17", bitWidth: "1", description: "Channel 5 Transfer Complete
              flag", name: TCIF5 }
                  { bitOffset: "18", bitWidth: "1", description: "Channel 5 Half Transfer Complete
              flag", name: HTIF5 }
                  { bitOffset: "19", bitWidth: "1", description: "Channel 5 Transfer Error
              flag", name: TEIF5 }
                  { bitOffset: "20", bitWidth: "1", description: "Channel 6 Global interrupt
              flag", name: GIF6 }
                  { bitOffset: "21", bitWidth: "1", description: "Channel 6 Transfer Complete
              flag", name: TCIF6 }
                  { bitOffset: "22", bitWidth: "1", description: "Channel 6 Half Transfer Complete
              flag", name: HTIF6 }
                  { bitOffset: "23", bitWidth: "1", description: "Channel 6 Transfer Error
              flag", name: TEIF6 }
                  { bitOffset: "24", bitWidth: "1", description: "Channel 7 Global interrupt
              flag", name: GIF7 }
                  { bitOffset: "25", bitWidth: "1", description: "Channel 7 Transfer Complete
              flag", name: TCIF7 }
                  { bitOffset: "26", bitWidth: "1", description: "Channel 7 Half Transfer Complete
              flag", name: HTIF7 }
                  { bitOffset: "27", bitWidth: "1", description: "Channel 7 Transfer Error
              flag", name: TEIF7 }
                ]
              }
              name: INTFR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x4"
              description: "DMA interrupt flag clear register
          (DMA_INTFCR)"
              displayName: INTFCR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel 1 Global interrupt
              clear", name: CGIF1 }
                  { bitOffset: "4", bitWidth: "1", description: "Channel 2 Global interrupt
              clear", name: CGIF2 }
                  { bitOffset: "8", bitWidth: "1", description: "Channel 3 Global interrupt
              clear", name: CGIF3 }
                  { bitOffset: "12", bitWidth: "1", description: "Channel 4 Global interrupt
              clear", name: CGIF4 }
                  { bitOffset: "16", bitWidth: "1", description: "Channel 5 Global interrupt
              clear", name: CGIF5 }
                  { bitOffset: "20", bitWidth: "1", description: "Channel 6 Global interrupt
              clear", name: CGIF6 }
                  { bitOffset: "24", bitWidth: "1", description: "Channel 7 Global interrupt
              clear", name: CGIF7 }
                  { bitOffset: "1", bitWidth: "1", description: "Channel 1 Transfer Complete
              clear", name: CTCIF1 }
                  { bitOffset: "5", bitWidth: "1", description: "Channel 2 Transfer Complete
              clear", name: CTCIF2 }
                  { bitOffset: "9", bitWidth: "1", description: "Channel 3 Transfer Complete
              clear", name: CTCIF3 }
                  { bitOffset: "13", bitWidth: "1", description: "Channel 4 Transfer Complete
              clear", name: CTCIF4 }
                  { bitOffset: "17", bitWidth: "1", description: "Channel 5 Transfer Complete
              clear", name: CTCIF5 }
                  { bitOffset: "21", bitWidth: "1", description: "Channel 6 Transfer Complete
              clear", name: CTCIF6 }
                  { bitOffset: "25", bitWidth: "1", description: "Channel 7 Transfer Complete
              clear", name: CTCIF7 }
                  { bitOffset: "2", bitWidth: "1", description: "Channel 1 Half Transfer
              clear", name: CHTIF1 }
                  { bitOffset: "6", bitWidth: "1", description: "Channel 2 Half Transfer
              clear", name: CHTIF2 }
                  { bitOffset: "10", bitWidth: "1", description: "Channel 3 Half Transfer
              clear", name: CHTIF3 }
                  { bitOffset: "14", bitWidth: "1", description: "Channel 4 Half Transfer
              clear", name: CHTIF4 }
                  { bitOffset: "18", bitWidth: "1", description: "Channel 5 Half Transfer
              clear", name: CHTIF5 }
                  { bitOffset: "22", bitWidth: "1", description: "Channel 6 Half Transfer
              clear", name: CHTIF6 }
                  { bitOffset: "26", bitWidth: "1", description: "Channel 7 Half Transfer
              clear", name: CHTIF7 }
                  { bitOffset: "3", bitWidth: "1", description: "Channel 1 Transfer Error
              clear", name: CTEIF1 }
                  { bitOffset: "7", bitWidth: "1", description: "Channel 2 Transfer Error
              clear", name: CTEIF2 }
                  { bitOffset: "11", bitWidth: "1", description: "Channel 3 Transfer Error
              clear", name: CTEIF3 }
                  { bitOffset: "15", bitWidth: "1", description: "Channel 4 Transfer Error
              clear", name: CTEIF4 }
                  { bitOffset: "19", bitWidth: "1", description: "Channel 5 Transfer Error
              clear", name: CTEIF5 }
                  { bitOffset: "23", bitWidth: "1", description: "Channel 6 Transfer Error
              clear", name: CTEIF6 }
                  { bitOffset: "27", bitWidth: "1", description: "Channel 7 Transfer Error
              clear", name: CTEIF7 }
                ]
              }
              name: INTFCR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR1
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "DMA channel 1 number of data
          register"
              displayName: CNTR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "DMA channel 1 peripheral address
          register"
              displayName: PADDR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "DMA channel 1 memory address
          register"
              displayName: MADDR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR2
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x20"
              description: "DMA channel 2 number of data
          register"
              displayName: CNTR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x24"
              description: "DMA channel 2 peripheral address
          register"
              displayName: PADDR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x28"
              description: "DMA channel 2 memory address
          register"
              displayName: MADDR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x30"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR3
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x34"
              description: "DMA channel 3 number of data
          register"
              displayName: CNTR3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x38"
              description: "DMA channel 3 peripheral address
          register"
              displayName: PADDR3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x3C"
              description: "DMA channel 3 memory address
          register"
              displayName: MADDR3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x44"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR4
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR4
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x48"
              description: "DMA channel 4 number of data
          register"
              displayName: CNTR4
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR4
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x4C"
              description: "DMA channel 4 peripheral address
          register"
              displayName: PADDR4
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR4
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x50"
              description: "DMA channel 4 memory address
          register"
              displayName: MADDR4
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR4
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x58"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR5
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR5
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x5C"
              description: "DMA channel 5 number of data
          register"
              displayName: CNTR5
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR5
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x60"
              description: "DMA channel 5 peripheral address
          register"
              displayName: PADDR5
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR5
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x64"
              description: "DMA channel 5 memory address
          register"
              displayName: MADDR5
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR5
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x6C"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR6
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR6
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x70"
              description: "DMA channel 6 number of data
          register"
              displayName: CNTR6
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR6
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x74"
              description: "DMA channel 6 peripheral address
          register"
              displayName: PADDR6
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR6
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x78"
              description: "DMA channel 6 memory address
          register"
              displayName: MADDR6
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR6
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x80"
              description: "DMA channel configuration register
          (DMA_CFGR)"
              displayName: CFGR7
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Channel enable", name: EN }
                  { bitOffset: "1", bitWidth: "1", description: "Transfer complete interrupt
              enable", name: TCIE }
                  { bitOffset: "2", bitWidth: "1", description: "Half Transfer interrupt
              enable", name: HTIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transfer error interrupt
              enable", name: TEIE }
                  { bitOffset: "4", bitWidth: "1", description: "Data transfer direction", name: DIR }
                  { bitOffset: "5", bitWidth: "1", description: "Circular mode", name: CIRC }
                  { bitOffset: "6", bitWidth: "1", description: "Peripheral increment mode", name: PINC }
                  { bitOffset: "7", bitWidth: "1", description: "Memory increment mode", name: MINC }
                  { bitOffset: "8", bitWidth: "2", description: "Peripheral size", name: PSIZE }
                  { bitOffset: "10", bitWidth: "2", description: "Memory size", name: MSIZE }
                  { bitOffset: "12", bitWidth: "2", description: "Channel Priority level", name: PL }
                  { bitOffset: "14", bitWidth: "1", description: "Memory to memory mode", name: MEM2MEM }
                ]
              }
              name: CFGR7
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x84"
              description: "DMA channel 7 number of data
          register"
              displayName: CNTR7
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Number of data to transfer"
                  name: NDT
                }
              }
              name: CNTR7
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x88"
              description: "DMA channel 7 peripheral address
          register"
              displayName: PADDR7
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Peripheral address"
                  name: PA
                }
              }
              name: PADDR7
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x8C"
              description: "DMA channel 7 memory address
          register"
              displayName: MADDR7
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Memory address"
                  name: MA
                }
              }
              name: MADDR7
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40002800"
        description: "Real time clock"
        groupName: RTC
        interrupt: [
          { description: "RTC global interrupt", name: RTC, value: "19" }
          { description: "RTC Alarms through EXTI line
        interrupt", name: RTCAlarm, value: "57" }
        ]
        name: RTC
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "RTC Control Register High"
              displayName: CTLRH
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Second interrupt Enable", name: SECIE }
                  { bitOffset: "1", bitWidth: "1", description: "Alarm interrupt Enable", name: ALRIE }
                  { bitOffset: "2", bitWidth: "1", description: "Overflow interrupt Enable", name: OWIE }
                ]
              }
              name: CTLRH
              resetValue: "0x0000"
              size: "0x20"
            }
            {
              addressOffset: "0x4"
              description: "RTC Control Register Low"
              displayName: CTLRL
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "1", description: "Second Flag", name: SECF }
                  { access: read-write, bitOffset: "1", bitWidth: "1", description: "Alarm Flag", name: ALRF }
                  { access: read-write, bitOffset: "2", bitWidth: "1", description: "Overflow Flag", name: OWF }
                  { access: read-write, bitOffset: "3", bitWidth: "1", description: "Registers Synchronized
              Flag", name: RSF }
                  { access: read-write, bitOffset: "4", bitWidth: "1", description: "Configuration Flag", name: CNF }
                  { access: read-only, bitOffset: "5", bitWidth: "1", description: "RTC operation OFF", name: RTOFF }
                ]
              }
              name: CTLRL
              resetValue: "0x0020"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0x8"
              description: "RTC Prescaler Load Register
          High"
              displayName: PSCRH
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "4"
                  description: "RTC Prescaler Load Register
              High"
                  name: PRLH
                }
              }
              name: PSCRH
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0xC"
              description: "RTC Prescaler Load Register
          Low"
              displayName: PSCRL
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "RTC Prescaler Divider Register
              Low"
                  name: PRLL
                }
              }
              name: PSCRL
              resetValue: "0x8000"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x10"
              description: "RTC Prescaler Divider Register
          High"
              displayName: DIVH
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "4"
                  description: "RTC prescaler divider register
              high"
                  name: DIVH
                }
              }
              name: DIVH
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x14"
              description: "RTC Prescaler Divider Register
          Low"
              displayName: DIVL
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "RTC prescaler divider register
              Low"
                  name: DIVL
                }
              }
              name: DIVL
              resetValue: "0x8000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "RTC Counter Register High"
              displayName: CNTH
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "RTC counter register high"
                  name: CNTH
                }
              }
              name: CNTH
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "RTC Counter Register Low"
              displayName: CNTL
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "RTC counter register Low"
                  name: CNTL
                }
              }
              name: CNTL
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0x20"
              description: "RTC Alarm Register High"
              displayName: ALRMH
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "RTC alarm register high"
                  name: ALRMH
                }
              }
              name: ALRMH
              resetValue: "0x0000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x24"
              description: "RTC Alarm Register Low"
              displayName: ALRML
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "RTC alarm register low"
                  name: ALRML
                }
              }
              name: ALRML
              resetValue: "0x0000"
              size: "0x10"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40006C00"
        description: "Backup registers"
        groupName: BKP
        name: BKP
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x4"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D1
                }
              }
              name: DATAR1
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D2
                }
              }
              name: DATAR2
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D3
                }
              }
              name: DATAR3
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR4
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D4
                }
              }
              name: DATAR4
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR5
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D5
                }
              }
              name: DATAR5
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR6
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D6
                }
              }
              name: DATAR6
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR7
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D7
                }
              }
              name: DATAR7
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x20"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR8
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D8
                }
              }
              name: DATAR8
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x24"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR9
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D9
                }
              }
              name: DATAR9
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x28"
              description: "Backup data register (BKP_DR)"
              displayName: DATAR10
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Backup data"
                  name: D10
                }
              }
              name: DATAR10
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x2C"
              description: "RTC clock calibration register
          (BKP_OCTLR)"
              displayName: OCTLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "7", description: "Calibration value", name: CAL }
                  { bitOffset: "7", bitWidth: "1", description: "Calibration Clock Output", name: CCO }
                  { bitOffset: "8", bitWidth: "1", description: "Alarm or second output
              enable", name: ASOE }
                  { bitOffset: "9", bitWidth: "1", description: "Alarm or second output
              selection", name: ASOS }
                ]
              }
              name: OCTLR
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x30"
              description: "Backup control register
          (BKP_TPCTLR)"
              displayName: TPCTLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Tamper pin enable", name: TPE }
                  { bitOffset: "1", bitWidth: "1", description: "Tamper pin active level", name: TPAL }
                ]
              }
              name: TPCTLR
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              addressOffset: "0x34"
              description: "BKP_TPCSR control/status register
          (BKP_CSR)"
              displayName: TPCSR
              fields:
              {
                field: [
                  { access: write-only, bitOffset: "0", bitWidth: "1", description: "Clear Tamper event", name: CTE }
                  { access: write-only, bitOffset: "1", bitWidth: "1", description: "Clear Tamper Interrupt", name: CTI }
                  { access: read-write, bitOffset: "2", bitWidth: "1", description: "Tamper Pin interrupt
              enable", name: TPIE }
                  { access: read-only, bitOffset: "8", bitWidth: "1", description: "Tamper Event Flag", name: TEF }
                  { access: read-only, bitOffset: "9", bitWidth: "1", description: "Tamper Interrupt Flag", name: TIF }
                ]
              }
              name: TPCSR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40003000"
        description: "Independent watchdog"
        groupName: IWDG
        name: IWDG
        registers:
        {
          register: [
            {
              access: write-only
              addressOffset: "0x0"
              description: "Key register (IWDG_CTLR)"
              displayName: CTLR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Key value"
                  name: KEY
                }
              }
              name: CTLR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "Prescaler register (IWDG_PSCR)"
              displayName: PSCR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "3"
                  description: "Prescaler divider"
                  name: PR
                }
              }
              name: PSCR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "Reload register (IWDG_RLDR)"
              displayName: RLDR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Watchdog counter reload
              value"
                  name: RL
                }
              }
              name: RLDR
              resetValue: "0x0FFF"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0xC"
              description: "Status register (IWDG_SR)"
              displayName: STATR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Watchdog prescaler value
              update", name: PVU }
                  { bitOffset: "1", bitWidth: "1", description: "Watchdog counter reload value
              update", name: RVU }
                ]
              }
              name: STATR
              resetValue: "0x0000"
              size: "0x10"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40002C00"
        description: "Window watchdog"
        groupName: WWDG
        interrupt:
        {
          description: "Window Watchdog interrupt"
          name: WWDG
          value: "16"
        }
        name: WWDG
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Control register (WWDG_CR)"
              displayName: CTLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "7", description: "7-bit counter (MSB to LSB)", name: T }
                  { bitOffset: "7", bitWidth: "1", description: "Activation bit", name: WDGA }
                ]
              }
              name: CTLR
              resetValue: "0x007F"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "Configuration register
          (WWDG_CFR)"
              displayName: CFGR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "7", description: "7-bit window value", name: W }
                  { bitOffset: "7", bitWidth: "2", description: "Timer Base", name: WDGTB }
                  { bitOffset: "9", bitWidth: "1", description: "Early Wakeup Interrupt", name: EWI }
                ]
              }
              name: CFGR
              resetValue: "0x007F"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "Status register (WWDG_SR)"
              displayName: STATR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "1"
                  description: "Early Wakeup Interrupt Flag"
                  name: WEIF
                }
              }
              name: STATR
              resetValue: "0x0000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40012C00"
        description: "Advanced timer"
        groupName: TIM
        interrupt: [
          { description: "TIM1 Break interrupt and TIM9 global
        interrupt", name: TIM1_BRK, value: "40" }
          { description: "TIM1 Update interrupt and TIM10 global
        interrupt", name: TIM1_UP, value: "41" }
          { description: "TIM1 Trigger and Commutation interrupts and
        TIM11 global interrupt", name: TIM1_TRG_COM, value: "42" }
          { description: "TIM1 Capture Compare interrupt", name: TIM1_CC, value: "43" }
        ]
        name: TIM1
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "control register 1"
              displayName: CTLR1
              fields:
              {
                field: [
                  { bitOffset: "8", bitWidth: "2", description: "Clock division", name: CKD }
                  { bitOffset: "7", bitWidth: "1", description: "Auto-reload preload enable", name: ARPE }
                  { bitOffset: "5", bitWidth: "2", description: "Center-aligned mode
              selection", name: CMS }
                  { bitOffset: "4", bitWidth: "1", description: Direction, name: DIR }
                  { bitOffset: "3", bitWidth: "1", description: "One-pulse mode", name: OPM }
                  { bitOffset: "2", bitWidth: "1", description: "Update request source", name: URS }
                  { bitOffset: "1", bitWidth: "1", description: "Update disable", name: UDIS }
                  { bitOffset: "0", bitWidth: "1", description: "Counter enable", name: CEN }
                ]
              }
              name: CTLR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "control register 2"
              displayName: CTLR2
              fields:
              {
                field: [
                  { bitOffset: "14", bitWidth: "1", description: "Output Idle state 4", name: OIS4 }
                  { bitOffset: "13", bitWidth: "1", description: "Output Idle state 3", name: OIS3N }
                  { bitOffset: "12", bitWidth: "1", description: "Output Idle state 3", name: OIS3 }
                  { bitOffset: "11", bitWidth: "1", description: "Output Idle state 2", name: OIS2N }
                  { bitOffset: "10", bitWidth: "1", description: "Output Idle state 2", name: OIS2 }
                  { bitOffset: "9", bitWidth: "1", description: "Output Idle state 1", name: OIS1N }
                  { bitOffset: "8", bitWidth: "1", description: "Output Idle state 1", name: OIS1 }
                  { bitOffset: "7", bitWidth: "1", description: "TI1 selection", name: TI1S }
                  { bitOffset: "4", bitWidth: "3", description: "Master mode selection", name: MMS }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/compare DMA
              selection", name: CCDS }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/compare control update
              selection", name: CCUS }
                  { bitOffset: "0", bitWidth: "1", description: "Capture/compare preloaded
              control", name: CCPC }
                ]
              }
              name: CTLR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "slave mode control register"
              displayName: SMCFGR
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "External trigger polarity", name: ETP }
                  { bitOffset: "14", bitWidth: "1", description: "External clock enable", name: ECE }
                  { bitOffset: "12", bitWidth: "2", description: "External trigger prescaler", name: ETPS }
                  { bitOffset: "8", bitWidth: "4", description: "External trigger filter", name: ETF }
                  { bitOffset: "7", bitWidth: "1", description: "Master/Slave mode", name: MSM }
                  { bitOffset: "4", bitWidth: "3", description: "Trigger selection", name: TS }
                  { bitOffset: "0", bitWidth: "3", description: "Slave mode selection", name: SMS }
                ]
              }
              name: SMCFGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "DMA/Interrupt enable register"
              displayName: DMAINTENR
              fields:
              {
                field: [
                  { bitOffset: "14", bitWidth: "1", description: "Trigger DMA request enable", name: TDE }
                  { bitOffset: "13", bitWidth: "1", description: "COM DMA request enable", name: COMDE }
                  { bitOffset: "12", bitWidth: "1", description: "Capture/Compare 4 DMA request
              enable", name: CC4DE }
                  { bitOffset: "11", bitWidth: "1", description: "Capture/Compare 3 DMA request
              enable", name: CC3DE }
                  { bitOffset: "10", bitWidth: "1", description: "Capture/Compare 2 DMA request
              enable", name: CC2DE }
                  { bitOffset: "9", bitWidth: "1", description: "Capture/Compare 1 DMA request
              enable", name: CC1DE }
                  { bitOffset: "8", bitWidth: "1", description: "Update DMA request enable", name: UDE }
                  { bitOffset: "6", bitWidth: "1", description: "Trigger interrupt enable", name: TIE }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/Compare 4 interrupt
              enable", name: CC4IE }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/Compare 3 interrupt
              enable", name: CC3IE }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/Compare 2 interrupt
              enable", name: CC2IE }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/Compare 1 interrupt
              enable", name: CC1IE }
                  { bitOffset: "0", bitWidth: "1", description: "Update interrupt enable", name: UIE }
                  { bitOffset: "7", bitWidth: "1", description: "Break interrupt enable", name: BIE }
                  { bitOffset: "5", bitWidth: "1", description: "COM interrupt enable", name: COMIE }
                ]
              }
              name: DMAINTENR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "status register"
              displayName: INTFR
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "1", description: "Capture/Compare 4 overcapture
              flag", name: CC4OF }
                  { bitOffset: "11", bitWidth: "1", description: "Capture/Compare 3 overcapture
              flag", name: CC3OF }
                  { bitOffset: "10", bitWidth: "1", description: "Capture/compare 2 overcapture
              flag", name: CC2OF }
                  { bitOffset: "9", bitWidth: "1", description: "Capture/Compare 1 overcapture
              flag", name: CC1OF }
                  { bitOffset: "7", bitWidth: "1", description: "Break interrupt flag", name: BIF }
                  { bitOffset: "6", bitWidth: "1", description: "Trigger interrupt flag", name: TIF }
                  { bitOffset: "5", bitWidth: "1", description: "COM interrupt flag", name: COMIF }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/Compare 4 interrupt
              flag", name: CC4IF }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/Compare 3 interrupt
              flag", name: CC3IF }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/Compare 2 interrupt
              flag", name: CC2IF }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/compare 1 interrupt
              flag", name: CC1IF }
                  { bitOffset: "0", bitWidth: "1", description: "Update interrupt flag", name: UIF }
                ]
              }
              name: INTFR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0x14"
              description: "event generation register"
              displayName: SWEVGR
              fields:
              {
                field: [
                  { bitOffset: "7", bitWidth: "1", description: "Break generation", name: BG }
                  { bitOffset: "6", bitWidth: "1", description: "Trigger generation", name: TG }
                  { bitOffset: "5", bitWidth: "1", description: "Capture/Compare control update
              generation", name: COMG }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/compare 4
              generation", name: CC4G }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/compare 3
              generation", name: CC3G }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/compare 2
              generation", name: CC2G }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/compare 1
              generation", name: CC1G }
                  { bitOffset: "0", bitWidth: "1", description: "Update generation", name: UG }
                ]
              }
              name: SWEVGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "capture/compare mode register (output
          mode)"
              displayName: CHCTLR1_Output
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Output Compare 2 clear
              enable", name: OC2CE }
                  { bitOffset: "12", bitWidth: "3", description: "Output Compare 2 mode", name: OC2M }
                  { bitOffset: "11", bitWidth: "1", description: "Output Compare 2 preload
              enable", name: OC2PE }
                  { bitOffset: "10", bitWidth: "1", description: "Output Compare 2 fast
              enable", name: OC2FE }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 2
              selection", name: CC2S }
                  { bitOffset: "7", bitWidth: "1", description: "Output Compare 1 clear
              enable", name: OC1CE }
                  { bitOffset: "4", bitWidth: "3", description: "Output Compare 1 mode", name: OC1M }
                  { bitOffset: "3", bitWidth: "1", description: "Output Compare 1 preload
              enable", name: OC1PE }
                  { bitOffset: "2", bitWidth: "1", description: "Output Compare 1 fast
              enable", name: OC1FE }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 1
              selection", name: CC1S }
                ]
              }
              name: CHCTLR1_Output
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              alternateRegister: CHCTLR1_Output
              description: "capture/compare mode register 1 (input
          mode)"
              displayName: CHCTLR1_Input
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "4", description: "Input capture 2 filter", name: IC2F }
                  { bitOffset: "10", bitWidth: "2", description: "Input capture 2 prescaler", name: IC2PSC }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 2
              selection", name: CC2S }
                  { bitOffset: "4", bitWidth: "4", description: "Input capture 1 filter", name: IC1F }
                  { bitOffset: "2", bitWidth: "2", description: "Input capture 1 prescaler", name: IC1PSC }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 1
              selection", name: CC1S }
                ]
              }
              name: CHCTLR1_Input
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "capture/compare mode register (output
          mode)"
              displayName: CHCTLR2_Output
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Output compare 4 clear
              enable", name: OC4CE }
                  { bitOffset: "12", bitWidth: "3", description: "Output compare 4 mode", name: OC4M }
                  { bitOffset: "11", bitWidth: "1", description: "Output compare 4 preload
              enable", name: OC4PE }
                  { bitOffset: "10", bitWidth: "1", description: "Output compare 4 fast
              enable", name: OC4FE }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 4
              selection", name: CC4S }
                  { bitOffset: "7", bitWidth: "1", description: "Output compare 3 clear
              enable", name: OC3CE }
                  { bitOffset: "4", bitWidth: "3", description: "Output compare 3 mode", name: OC3M }
                  { bitOffset: "3", bitWidth: "1", description: "Output compare 3 preload
              enable", name: OC3PE }
                  { bitOffset: "2", bitWidth: "1", description: "Output compare 3 fast
              enable", name: OC3FE }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 3
              selection", name: CC3S }
                ]
              }
              name: CHCTLR2_Output
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              alternateRegister: CHCTLR2_Output
              description: "capture/compare mode register 2 (input
          mode)"
              displayName: CHCTLR2_Input
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "4", description: "Input capture 4 filter", name: IC4F }
                  { bitOffset: "10", bitWidth: "2", description: "Input capture 4 prescaler", name: IC4PSC }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 4
              selection", name: CC4S }
                  { bitOffset: "4", bitWidth: "4", description: "Input capture 3 filter", name: IC3F }
                  { bitOffset: "2", bitWidth: "2", description: "Input capture 3 prescaler", name: IC3PSC }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/compare 3
              selection", name: CC3S }
                ]
              }
              name: CHCTLR2_Input
              resetValue: "0x00000000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x20"
              description: "capture/compare enable
          register"
              displayName: CCER
              fields:
              {
                field: [
                  { bitOffset: "13", bitWidth: "1", description: "Capture/Compare 3 output
              Polarity", name: CC4P }
                  { bitOffset: "12", bitWidth: "1", description: "Capture/Compare 4 output
              enable", name: CC4E }
                  { bitOffset: "11", bitWidth: "1", description: "Capture/Compare 3 output
              Polarity", name: CC3NP }
                  { bitOffset: "10", bitWidth: "1", description: "Capture/Compare 3 complementary output
              enable", name: CC3NE }
                  { bitOffset: "9", bitWidth: "1", description: "Capture/Compare 3 output
              Polarity", name: CC3P }
                  { bitOffset: "8", bitWidth: "1", description: "Capture/Compare 3 output
              enable", name: CC3E }
                  { bitOffset: "7", bitWidth: "1", description: "Capture/Compare 2 output
              Polarity", name: CC2NP }
                  { bitOffset: "6", bitWidth: "1", description: "Capture/Compare 2 complementary output
              enable", name: CC2NE }
                  { bitOffset: "5", bitWidth: "1", description: "Capture/Compare 2 output
              Polarity", name: CC2P }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/Compare 2 output
              enable", name: CC2E }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/Compare 1 output
              Polarity", name: CC1NP }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/Compare 1 complementary output
              enable", name: CC1NE }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/Compare 1 output
              Polarity", name: CC1P }
                  { bitOffset: "0", bitWidth: "1", description: "Capture/Compare 1 output
              enable", name: CC1E }
                ]
              }
              name: CCER
              resetValue: "0x0000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x24"
              description: counter
              displayName: CNT
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "counter value"
                  name: CNT
                }
              }
              name: CNT
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x28"
              description: prescaler
              displayName: PSC
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Prescaler value"
                  name: PSC
                }
              }
              name: PSC
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x2C"
              description: "auto-reload register"
              displayName: ATRLR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Auto-reload value"
                  name: ARR
                }
              }
              name: ATRLR
              resetValue: "0xFFFF"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x30"
              description: "repetition counter register"
              displayName: RPTCR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "8"
                  description: "Repetition counter value"
                  name: REP
                }
              }
              name: RPTCR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x34"
              description: "capture/compare register 1"
              displayName: CH1CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare 1 value"
                  name: CCR1
                }
              }
              name: CH1CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x38"
              description: "capture/compare register 2"
              displayName: CH2CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare 2 value"
                  name: CCR2
                }
              }
              name: CH2CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x3C"
              description: "capture/compare register 3"
              displayName: CH3CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare value"
                  name: CCR3
                }
              }
              name: CH3CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x40"
              description: "capture/compare register 4"
              displayName: CH4CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare value"
                  name: CCR4
                }
              }
              name: CH4CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x48"
              description: "DMA control register"
              displayName: DMACFGR
              fields:
              {
                field: [
                  { bitOffset: "8", bitWidth: "5", description: "DMA burst length", name: DBL }
                  { bitOffset: "0", bitWidth: "5", description: "DMA base address", name: DBA }
                ]
              }
              name: DMACFGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4C"
              description: "DMA address for full transfer"
              displayName: DMAR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "DMA register for burst
              accesses"
                  name: DMAB
                }
              }
              name: DMAR
              resetValue: "0x0000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x44"
              description: "break and dead-time register"
              displayName: BDTR
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Main output enable", name: MOE }
                  { bitOffset: "14", bitWidth: "1", description: "Automatic output enable", name: AOE }
                  { bitOffset: "13", bitWidth: "1", description: "Break polarity", name: BKP }
                  { bitOffset: "12", bitWidth: "1", description: "Break enable", name: BKE }
                  { bitOffset: "11", bitWidth: "1", description: "Off-state selection for Run
              mode", name: OSSR }
                  { bitOffset: "10", bitWidth: "1", description: "Off-state selection for Idle
              mode", name: OSSI }
                  { bitOffset: "8", bitWidth: "2", description: "Lock configuration", name: LOCK }
                  { bitOffset: "0", bitWidth: "8", description: "Dead-time generator setup", name: DTG }
                ]
              }
              name: BDTR
              resetValue: "0x0000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40000000"
        description: "General purpose timer"
        groupName: TIM
        interrupt:
        {
          description: "TIM2 global interrupt"
          name: TIM2
          value: "44"
        }
        name: TIM2
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "control register 1"
              displayName: CTLR1
              fields:
              {
                field: [
                  { bitOffset: "8", bitWidth: "2", description: "Clock division", name: CKD }
                  { bitOffset: "7", bitWidth: "1", description: "Auto-reload preload enable", name: ARPE }
                  { bitOffset: "5", bitWidth: "2", description: "Center-aligned mode
              selection", name: CMS }
                  { bitOffset: "4", bitWidth: "1", description: Direction, name: DIR }
                  { bitOffset: "3", bitWidth: "1", description: "One-pulse mode", name: OPM }
                  { bitOffset: "2", bitWidth: "1", description: "Update request source", name: URS }
                  { bitOffset: "1", bitWidth: "1", description: "Update disable", name: UDIS }
                  { bitOffset: "0", bitWidth: "1", description: "Counter enable", name: CEN }
                ]
              }
              name: CTLR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "control register 2"
              displayName: CTLR2
              fields:
              {
                field: [
                  { bitOffset: "7", bitWidth: "1", description: "TI1 selection", name: TI1S }
                  { bitOffset: "4", bitWidth: "3", description: "Master mode selection", name: MMS }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/compare DMA
              selection", name: CCDS }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/compare control update
              selection", name: CCUS }
                  { bitOffset: "0", bitWidth: "1", description: "Capture/compare preloaded
              control", name: CCPC }
                ]
              }
              name: CTLR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "slave mode control register"
              displayName: SMCFGR
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "External trigger polarity", name: ETP }
                  { bitOffset: "14", bitWidth: "1", description: "External clock enable", name: ECE }
                  { bitOffset: "12", bitWidth: "2", description: "External trigger prescaler", name: ETPS }
                  { bitOffset: "8", bitWidth: "4", description: "External trigger filter", name: ETF }
                  { bitOffset: "7", bitWidth: "1", description: "Master/Slave mode", name: MSM }
                  { bitOffset: "4", bitWidth: "3", description: "Trigger selection", name: TS }
                  { bitOffset: "0", bitWidth: "3", description: "Slave mode selection", name: SMS }
                ]
              }
              name: SMCFGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "DMA/Interrupt enable register"
              displayName: DMAINTENR
              fields:
              {
                field: [
                  { bitOffset: "14", bitWidth: "1", description: "Trigger DMA request enable", name: TDE }
                  { bitOffset: "12", bitWidth: "1", description: "Capture/Compare 4 DMA request
              enable", name: CC4DE }
                  { bitOffset: "11", bitWidth: "1", description: "Capture/Compare 3 DMA request
              enable", name: CC3DE }
                  { bitOffset: "10", bitWidth: "1", description: "Capture/Compare 2 DMA request
              enable", name: CC2DE }
                  { bitOffset: "9", bitWidth: "1", description: "Capture/Compare 1 DMA request
              enable", name: CC1DE }
                  { bitOffset: "8", bitWidth: "1", description: "Update DMA request enable", name: UDE }
                  { bitOffset: "6", bitWidth: "1", description: "Trigger interrupt enable", name: TIE }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/Compare 4 interrupt
              enable", name: CC4IE }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/Compare 3 interrupt
              enable", name: CC3IE }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/Compare 2 interrupt
              enable", name: CC2IE }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/Compare 1 interrupt
              enable", name: CC1IE }
                  { bitOffset: "0", bitWidth: "1", description: "Update interrupt enable", name: UIE }
                ]
              }
              name: DMAINTENR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0x10"
              description: "status register"
              displayName: INTFR
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "1", description: "Capture/Compare 4 overcapture
              flag", name: CC4OF }
                  { bitOffset: "11", bitWidth: "1", description: "Capture/Compare 3 overcapture
              flag", name: CC3OF }
                  { bitOffset: "10", bitWidth: "1", description: "Capture/compare 2 overcapture
              flag", name: CC2OF }
                  { bitOffset: "9", bitWidth: "1", description: "Capture/Compare 1 overcapture
              flag", name: CC1OF }
                  { bitOffset: "6", bitWidth: "1", description: "Trigger interrupt flag", name: TIF }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/Compare 4 interrupt
              flag", name: CC4IF }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/Compare 3 interrupt
              flag", name: CC3IF }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/Compare 2 interrupt
              flag", name: CC2IF }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/compare 1 interrupt
              flag", name: CC1IF }
                  { bitOffset: "0", bitWidth: "1", description: "Update interrupt flag", name: UIF }
                ]
              }
              name: INTFR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0x14"
              description: "event generation register"
              displayName: SWEVGR
              fields:
              {
                field: [
                  { bitOffset: "7", bitWidth: "1", description: "Break generation", name: BG }
                  { bitOffset: "6", bitWidth: "1", description: "Trigger generation", name: TG }
                  { bitOffset: "5", bitWidth: "1", description: "Capture/Compare control update
              generation", name: COMG }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/compare 4
              generation", name: CC4G }
                  { bitOffset: "3", bitWidth: "1", description: "Capture/compare 3
              generation", name: CC3G }
                  { bitOffset: "2", bitWidth: "1", description: "Capture/compare 2
              generation", name: CC2G }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/compare 1
              generation", name: CC1G }
                  { bitOffset: "0", bitWidth: "1", description: "Update generation", name: UG }
                ]
              }
              name: SWEVGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "capture/compare mode register 1 (output
          mode)"
              displayName: CHCTLR1_Output
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Output compare 2 clear
              enable", name: OC2CE }
                  { bitOffset: "12", bitWidth: "3", description: "Output compare 2 mode", name: OC2M }
                  { bitOffset: "11", bitWidth: "1", description: "Output compare 2 preload
              enable", name: OC2PE }
                  { bitOffset: "10", bitWidth: "1", description: "Output compare 2 fast
              enable", name: OC2FE }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 2
              selection", name: CC2S }
                  { bitOffset: "7", bitWidth: "1", description: "Output compare 1 clear
              enable", name: OC1CE }
                  { bitOffset: "4", bitWidth: "3", description: "Output compare 1 mode", name: OC1M }
                  { bitOffset: "3", bitWidth: "1", description: "Output compare 1 preload
              enable", name: OC1PE }
                  { bitOffset: "2", bitWidth: "1", description: "Output compare 1 fast
              enable", name: OC1FE }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 1
              selection", name: CC1S }
                ]
              }
              name: CHCTLR1_Output
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              alternateRegister: CHCTLR1_Output
              description: "capture/compare mode register 1 (input
          mode)"
              displayName: CHCTLR1_Input
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "4", description: "Input capture 2 filter", name: IC2F }
                  { bitOffset: "10", bitWidth: "2", description: "Input capture 2 prescaler", name: IC2PSC }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/compare 2
              selection", name: CC2S }
                  { bitOffset: "4", bitWidth: "4", description: "Input capture 1 filter", name: IC1F }
                  { bitOffset: "2", bitWidth: "2", description: "Input capture 1 prescaler", name: IC1PSC }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 1
              selection", name: CC1S }
                ]
              }
              name: CHCTLR1_Input
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "capture/compare mode register 2 (output
          mode)"
              displayName: CHCTLR2_Output
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Output compare 4 clear
              enable", name: OC4CE }
                  { bitOffset: "12", bitWidth: "3", description: "Output compare 4 mode", name: OC4M }
                  { bitOffset: "11", bitWidth: "1", description: "Output compare 4 preload
              enable", name: OC4PE }
                  { bitOffset: "10", bitWidth: "1", description: "Output compare 4 fast
              enable", name: OC4FE }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 4
              selection", name: CC4S }
                  { bitOffset: "7", bitWidth: "1", description: "Output compare 3 clear
              enable", name: OC3CE }
                  { bitOffset: "4", bitWidth: "3", description: "Output compare 3 mode", name: OC3M }
                  { bitOffset: "3", bitWidth: "1", description: "Output compare 3 preload
              enable", name: OC3PE }
                  { bitOffset: "2", bitWidth: "1", description: "Output compare 3 fast
              enable", name: OC3FE }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 3
              selection", name: CC3S }
                ]
              }
              name: CHCTLR2_Output
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              alternateRegister: CHCTLR2_Output
              description: "capture/compare mode register 2 (input
          mode)"
              displayName: CHCTLR2_Input
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "4", description: "Input capture 4 filter", name: IC4F }
                  { bitOffset: "10", bitWidth: "2", description: "Input capture 4 prescaler", name: IC4PSC }
                  { bitOffset: "8", bitWidth: "2", description: "Capture/Compare 4
              selection", name: CC4S }
                  { bitOffset: "4", bitWidth: "4", description: "Input capture 3 filter", name: IC3F }
                  { bitOffset: "2", bitWidth: "2", description: "Input capture 3 prescaler", name: IC3PSC }
                  { bitOffset: "0", bitWidth: "2", description: "Capture/Compare 3
              selection", name: CC3S }
                ]
              }
              name: CHCTLR2_Input
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x20"
              description: "capture/compare enable
          register"
              displayName: CCER
              fields:
              {
                field: [
                  { bitOffset: "13", bitWidth: "1", description: "Capture/Compare 3 output
              Polarity", name: CC4P }
                  { bitOffset: "12", bitWidth: "1", description: "Capture/Compare 4 output
              enable", name: CC4E }
                  { bitOffset: "9", bitWidth: "1", description: "Capture/Compare 3 output
              Polarity", name: CC3P }
                  { bitOffset: "8", bitWidth: "1", description: "Capture/Compare 3 output
              enable", name: CC3E }
                  { bitOffset: "5", bitWidth: "1", description: "Capture/Compare 2 output
              Polarity", name: CC2P }
                  { bitOffset: "4", bitWidth: "1", description: "Capture/Compare 2 output
              enable", name: CC2E }
                  { bitOffset: "1", bitWidth: "1", description: "Capture/Compare 1 output
              Polarity", name: CC1P }
                  { bitOffset: "0", bitWidth: "1", description: "Capture/Compare 1 output
              enable", name: CC1E }
                ]
              }
              name: CCER
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x24"
              description: counter
              displayName: CNT
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "counter value"
                  name: CNT
                }
              }
              name: CNT
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x28"
              description: prescaler
              displayName: PSC
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Prescaler value"
                  name: PSC
                }
              }
              name: PSC
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x2C"
              description: "auto-reload register"
              displayName: ATRLR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Auto-reload value"
                  name: ARR
                }
              }
              name: ATRLR
              resetValue: "0xFFFF"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x34"
              description: "capture/compare register 1"
              displayName: CH1CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare 1 value"
                  name: CCR1
                }
              }
              name: CH1CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x38"
              description: "capture/compare register 2"
              displayName: CH2CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare 2 value"
                  name: CCR2
                }
              }
              name: CH2CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x3C"
              description: "capture/compare register 3"
              displayName: CH3CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare value"
                  name: CCR3
                }
              }
              name: CH3CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x40"
              description: "capture/compare register 4"
              displayName: CH4CVR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Capture/Compare value"
                  name: CCR4
                }
              }
              name: CH4CVR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x48"
              description: "DMA control register"
              displayName: DMACFGR
              fields:
              {
                field: [
                  { bitOffset: "8", bitWidth: "5", description: "DMA burst length", name: DBL }
                  { bitOffset: "0", bitWidth: "5", description: "DMA base address", name: DBA }
                ]
              }
              name: DMACFGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4C"
              description: "DMA address for full transfer"
              displayName: DMAADR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "DMA register for burst
              accesses"
                  name: DMAB
                }
              }
              name: DMAADR
              resetValue: "0x0000"
              size: "0x10"
            }
          ]
        }
      }
      {
        _attrs:
        {
          derivedFrom: TIM2
        }
        baseAddress: "0x40000400"
        interrupt:
        {
          description: "TIM3 global interrupt"
          name: TIM3
          value: "45"
        }
        name: TIM3
      }
      {
        _attrs:
        {
          derivedFrom: TIM2
        }
        baseAddress: "0x40000800"
        interrupt:
        {
          description: "TIM4 global interrupt"
          name: TIM4
          value: "46"
        }
        name: TIM4
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40005400"
        description: "Inter integrated circuit"
        groupName: I2C
        interrupt: [
          { description: "I2C1 event interrupt", name: I2C1_EV, value: "47" }
          { description: "I2C1 error interrupt", name: I2C1_ER, value: "48" }
        ]
        name: I2C1
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Control register 1"
              displayName: CTLR1
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Software reset", name: SWRST }
                  { bitOffset: "13", bitWidth: "1", description: "SMBus alert", name: ALERT }
                  { bitOffset: "12", bitWidth: "1", description: "Packet error checking", name: PEC }
                  { bitOffset: "11", bitWidth: "1", description: "Acknowledge/PEC Position (for data
              reception)", name: POS }
                  { bitOffset: "10", bitWidth: "1", description: "Acknowledge enable", name: ACK }
                  { bitOffset: "9", bitWidth: "1", description: "Stop generation", name: STOP }
                  { bitOffset: "8", bitWidth: "1", description: "Start generation", name: START }
                  { bitOffset: "7", bitWidth: "1", description: "Clock stretching disable (Slave
              mode)", name: NOSTRETCH }
                  { bitOffset: "6", bitWidth: "1", description: "General call enable", name: ENGC }
                  { bitOffset: "5", bitWidth: "1", description: "PEC enable", name: ENPEC }
                  { bitOffset: "4", bitWidth: "1", description: "ARP enable", name: ENARP }
                  { bitOffset: "3", bitWidth: "1", description: "SMBus type", name: SMBTYPE }
                  { bitOffset: "1", bitWidth: "1", description: "SMBus mode", name: SMBUS }
                  { bitOffset: "0", bitWidth: "1", description: "Peripheral enable", name: PE }
                ]
              }
              name: CTLR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "Control register 2"
              displayName: CTLR2
              fields:
              {
                field: [
                  { bitOffset: "12", bitWidth: "1", description: "DMA last transfer", name: LAST }
                  { bitOffset: "11", bitWidth: "1", description: "DMA requests enable", name: DMAEN }
                  { bitOffset: "10", bitWidth: "1", description: "Buffer interrupt enable", name: ITBUFEN }
                  { bitOffset: "9", bitWidth: "1", description: "Event interrupt enable", name: ITEVTEN }
                  { bitOffset: "8", bitWidth: "1", description: "Error interrupt enable", name: ITERREN }
                  { bitOffset: "0", bitWidth: "6", description: "Peripheral clock frequency", name: FREQ }
                ]
              }
              name: CTLR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "Own address register 1"
              displayName: OADDR1
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Addressing mode (slave
              mode)", name: ADDMODE }
                  { bitOffset: "8", bitWidth: "2", description: "Interface address", name: ADD9_8 }
                  { bitOffset: "1", bitWidth: "7", description: "Interface address", name: ADD7_1 }
                  { bitOffset: "0", bitWidth: "1", description: "Interface address", name: ADD0 }
                ]
              }
              name: OADDR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "Own address register 2"
              displayName: OADDR2
              fields:
              {
                field: [
                  { bitOffset: "1", bitWidth: "7", description: "Interface address", name: ADD2 }
                  { bitOffset: "0", bitWidth: "1", description: "Dual addressing mode
              enable", name: ENDUAL }
                ]
              }
              name: OADDR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "Data register"
              displayName: DATAR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "8"
                  description: "8-bit data register"
                  name: DR
                }
              }
              name: DATAR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              addressOffset: "0x14"
              description: "Status register 1"
              displayName: STAR1
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "15", bitWidth: "1", description: "SMBus alert", name: SMBALERT }
                  { access: read-write, bitOffset: "14", bitWidth: "1", description: "Timeout or Tlow error", name: TIMEOUT }
                  { access: read-write, bitOffset: "12", bitWidth: "1", description: "PEC Error in reception", name: PECERR }
                  { access: read-write, bitOffset: "11", bitWidth: "1", description: "Overrun/Underrun", name: OVR }
                  { access: read-write, bitOffset: "10", bitWidth: "1", description: "Acknowledge failure", name: AF }
                  { access: read-write, bitOffset: "9", bitWidth: "1", description: "Arbitration lost (master
              mode)", name: ARLO }
                  { access: read-write, bitOffset: "8", bitWidth: "1", description: "Bus error", name: BERR }
                  { access: read-only, bitOffset: "7", bitWidth: "1", description: "Data register empty
              (transmitters)", name: TxE }
                  { access: read-only, bitOffset: "6", bitWidth: "1", description: "Data register not empty
              (receivers)", name: RxNE }
                  { access: read-only, bitOffset: "4", bitWidth: "1", description: "Stop detection (slave
              mode)", name: STOPF }
                  { access: read-only, bitOffset: "3", bitWidth: "1", description: "10-bit header sent (Master
              mode)", name: ADD10 }
                  { access: read-only, bitOffset: "2", bitWidth: "1", description: "Byte transfer finished", name: BTF }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "Address sent (master mode)/matched
              (slave mode)", name: ADDR }
                  { access: read-only, bitOffset: "0", bitWidth: "1", description: "Start bit (Master mode)", name: SB }
                ]
              }
              name: STAR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x18"
              description: "Status register 2"
              displayName: STAR2
              fields:
              {
                field: [
                  { bitOffset: "8", bitWidth: "8", description: "acket error checking
              register", name: PEC }
                  { bitOffset: "7", bitWidth: "1", description: "Dual flag (Slave mode)", name: DUALF }
                  { bitOffset: "6", bitWidth: "1", description: "SMBus host header (Slave
              mode)", name: SMBHOST }
                  { bitOffset: "5", bitWidth: "1", description: "SMBus device default address (Slave
              mode)", name: SMBDEFAULT }
                  { bitOffset: "4", bitWidth: "1", description: "General call address (Slave
              mode)", name: GENCALL }
                  { bitOffset: "2", bitWidth: "1", description: "Transmitter/receiver", name: TRA }
                  { bitOffset: "1", bitWidth: "1", description: "Bus busy", name: BUSY }
                  { bitOffset: "0", bitWidth: "1", description: "Master/slave", name: MSL }
                ]
              }
              name: STAR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "Clock control register"
              displayName: CKCFGR
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "I2C master mode selection", name: F_S }
                  { bitOffset: "14", bitWidth: "1", description: "Fast mode duty cycle", name: DUTY }
                  { bitOffset: "0", bitWidth: "12", description: "Clock control register in Fast/Standard
              mode (Master mode)", name: CCR }
                ]
              }
              name: CKCFGR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x20"
              description: "risetime register"
              displayName: RTR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "6"
                  description: "Maximum rise time in Fast/Standard mode
              (Master mode)"
                  name: TRISE
                }
              }
              name: RTR
              resetValue: "0x0002"
              size: "0x10"
            }
          ]
        }
      }
      {
        _attrs:
        {
          derivedFrom: I2C1
        }
        baseAddress: "0x40005800"
        interrupt: [
          { description: "I2C2 event interrupt", name: I2C2_EV, value: "49" }
          { description: "I2C2 error interrupt", name: I2C2_ER, value: "50" }
        ]
        name: I2C2
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40013000"
        description: "Serial peripheral interface"
        groupName: SPI
        interrupt:
        {
          description: "SPI1 global interrupt"
          name: SPI1
          value: "51"
        }
        name: SPI1
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "control register 1"
              displayName: CTLR1
              fields:
              {
                field: [
                  { bitOffset: "15", bitWidth: "1", description: "Bidirectional data mode
              enable", name: BIDIMODE }
                  { bitOffset: "14", bitWidth: "1", description: "Output enable in bidirectional
              mode", name: BIDIOE }
                  { bitOffset: "13", bitWidth: "1", description: "Hardware CRC calculation
              enable", name: CRCEN }
                  { bitOffset: "12", bitWidth: "1", description: "CRC transfer next", name: CRCNEXT }
                  { bitOffset: "11", bitWidth: "1", description: "Data frame format", name: DFF }
                  { bitOffset: "10", bitWidth: "1", description: "Receive only", name: RXONLY }
                  { bitOffset: "9", bitWidth: "1", description: "Software slave management", name: SSM }
                  { bitOffset: "8", bitWidth: "1", description: "Internal slave select", name: SSI }
                  { bitOffset: "7", bitWidth: "1", description: "Frame format", name: LSBFIRST }
                  { bitOffset: "6", bitWidth: "1", description: "SPI enable", name: SPE }
                  { bitOffset: "3", bitWidth: "3", description: "Baud rate control", name: BR }
                  { bitOffset: "2", bitWidth: "1", description: "Master selection", name: MSTR }
                  { bitOffset: "1", bitWidth: "1", description: "Clock polarity", name: CPOL }
                  { bitOffset: "0", bitWidth: "1", description: "Clock phase", name: CPHA }
                ]
              }
              name: CTLR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "control register 2"
              displayName: CTLR2
              fields:
              {
                field: [
                  { bitOffset: "7", bitWidth: "1", description: "Tx buffer empty interrupt
              enable", name: TXEIE }
                  { bitOffset: "6", bitWidth: "1", description: "RX buffer not empty interrupt
              enable", name: RXNEIE }
                  { bitOffset: "5", bitWidth: "1", description: "Error interrupt enable", name: ERRIE }
                  { bitOffset: "2", bitWidth: "1", description: "SS output enable", name: SSOE }
                  { bitOffset: "1", bitWidth: "1", description: "Tx buffer DMA enable", name: TXDMAEN }
                  { bitOffset: "0", bitWidth: "1", description: "Rx buffer DMA enable", name: RXDMAEN }
                ]
              }
              name: CTLR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              addressOffset: "0x8"
              description: "status register"
              displayName: STATR
              fields:
              {
                field: [
                  { access: read-only, bitOffset: "7", bitWidth: "1", description: "Busy flag", name: BSY }
                  { access: read-only, bitOffset: "6", bitWidth: "1", description: "Overrun flag", name: OVR }
                  { access: read-only, bitOffset: "5", bitWidth: "1", description: "Mode fault", name: MODF }
                  { access: read-write, bitOffset: "4", bitWidth: "1", description: "CRC error flag", name: CRCERR }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "Transmit buffer empty", name: TXE }
                  { access: read-only, bitOffset: "0", bitWidth: "1", description: "Receive buffer not empty", name: RXNE }
                ]
              }
              name: STATR
              resetValue: "0x0002"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "data register"
              displayName: DATAR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Data register"
                  name: DATAR
                }
              }
              name: DATAR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "CRCR polynomial register"
              displayName: CRCR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "CRC polynomial register"
                  name: CRCPOLY
                }
              }
              name: CRCR
              resetValue: "0x0007"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x14"
              description: "RX CRC register"
              displayName: RCRCR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Rx CRC register"
                  name: RxCRC
                }
              }
              name: RCRCR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x18"
              description: "TX CRC register"
              displayName: TCRCR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Tx CRC register"
                  name: TxCRC
                }
              }
              name: TCRCR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: write-only
              addressOffset: "0x24"
              description: "High speed control register"
              displayName: HSCR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "1"
                  description: "High speed read mode enable bit"
                  name: HSRXEN
                }
              }
              name: HSCR
              resetValue: "0x0000"
              size: "0x10"
            }
          ]
        }
      }
      {
        _attrs:
        {
          derivedFrom: SPI1
        }
        baseAddress: "0x40003800"
        interrupt:
        {
          description: "SPI2 global interrupt"
          name: SPI2
          value: "52"
        }
        name: SPI2
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40013800"
        description: "Universal synchronous asynchronous receiver
      transmitter"
        groupName: USART
        interrupt:
        {
          description: "USART1 global interrupt"
          name: USART1
          value: "53"
        }
        name: USART1
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Status register"
              displayName: STATR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "9", bitWidth: "1", description: "CTS flag", name: CTS }
                  { access: read-write, bitOffset: "8", bitWidth: "1", description: "LIN break detection flag", name: LBD }
                  { access: read-only, bitOffset: "7", bitWidth: "1", description: "Transmit data register
              empty", name: TXE }
                  { access: read-write, bitOffset: "6", bitWidth: "1", description: "Transmission complete", name: TC }
                  { access: read-write, bitOffset: "5", bitWidth: "1", description: "Read data register not
              empty", name: RXNE }
                  { access: read-only, bitOffset: "4", bitWidth: "1", description: "IDLE line detected", name: IDLE }
                  { access: read-only, bitOffset: "3", bitWidth: "1", description: "Overrun error", name: ORE }
                  { access: read-only, bitOffset: "2", bitWidth: "1", description: "Noise error flag", name: NE }
                  { access: read-only, bitOffset: "1", bitWidth: "1", description: "Framing error", name: FE }
                  { access: read-only, bitOffset: "0", bitWidth: "1", description: "Parity error", name: PE }
                ]
              }
              name: STATR
              resetValue: "0x000000C0"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "Data register"
              displayName: DATAR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "9"
                  description: "Data value"
                  name: DR
                }
              }
              name: DATAR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "Baud rate register"
              displayName: BRR
              fields:
              {
                field: [
                  { bitOffset: "4", bitWidth: "12", description: "mantissa of USARTDIV", name: DIV_Mantissa }
                  { bitOffset: "0", bitWidth: "4", description: "fraction of USARTDIV", name: DIV_Fraction }
                ]
              }
              name: BRR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "Control register 1"
              displayName: CTLR1
              fields:
              {
                field: [
                  { bitOffset: "13", bitWidth: "1", description: "USART enable", name: UE }
                  { bitOffset: "12", bitWidth: "1", description: "Word length", name: M }
                  { bitOffset: "11", bitWidth: "1", description: "Wakeup method", name: WAKE }
                  { bitOffset: "10", bitWidth: "1", description: "Parity control enable", name: PCE }
                  { bitOffset: "9", bitWidth: "1", description: "Parity selection", name: PS }
                  { bitOffset: "8", bitWidth: "1", description: "PE interrupt enable", name: PEIE }
                  { bitOffset: "7", bitWidth: "1", description: "TXE interrupt enable", name: TXEIE }
                  { bitOffset: "6", bitWidth: "1", description: "Transmission complete interrupt
              enable", name: TCIE }
                  { bitOffset: "5", bitWidth: "1", description: "RXNE interrupt enable", name: RXNEIE }
                  { bitOffset: "4", bitWidth: "1", description: "IDLE interrupt enable", name: IDLEIE }
                  { bitOffset: "3", bitWidth: "1", description: "Transmitter enable", name: TE }
                  { bitOffset: "2", bitWidth: "1", description: "Receiver enable", name: RE }
                  { bitOffset: "1", bitWidth: "1", description: "Receiver wakeup", name: RWU }
                  { bitOffset: "0", bitWidth: "1", description: "Send break", name: SBK }
                ]
              }
              name: CTLR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "Control register 2"
              displayName: CTLR2
              fields:
              {
                field: [
                  { bitOffset: "14", bitWidth: "1", description: "LIN mode enable", name: LINEN }
                  { bitOffset: "12", bitWidth: "2", description: "STOP bits", name: STOP }
                  { bitOffset: "11", bitWidth: "1", description: "Clock enable", name: CLKEN }
                  { bitOffset: "10", bitWidth: "1", description: "Clock polarity", name: CPOL }
                  { bitOffset: "9", bitWidth: "1", description: "Clock phase", name: CPHA }
                  { bitOffset: "8", bitWidth: "1", description: "Last bit clock pulse", name: LBCL }
                  { bitOffset: "6", bitWidth: "1", description: "LIN break detection interrupt
              enable", name: LBDIE }
                  { bitOffset: "5", bitWidth: "1", description: "lin break detection length", name: LBDL }
                  { bitOffset: "0", bitWidth: "4", description: "Address of the USART node", name: ADD }
                ]
              }
              name: CTLR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "Control register 3"
              displayName: CTLR3
              fields:
              {
                field: [
                  { bitOffset: "10", bitWidth: "1", description: "CTS interrupt enable", name: CTSIE }
                  { bitOffset: "9", bitWidth: "1", description: "CTS enable", name: CTSE }
                  { bitOffset: "8", bitWidth: "1", description: "RTS enable", name: RTSE }
                  { bitOffset: "7", bitWidth: "1", description: "DMA enable transmitter", name: DMAT }
                  { bitOffset: "6", bitWidth: "1", description: "DMA enable receiver", name: DMAR }
                  { bitOffset: "5", bitWidth: "1", description: "Smartcard mode enable", name: SCEN }
                  { bitOffset: "4", bitWidth: "1", description: "Smartcard NACK enable", name: NACK }
                  { bitOffset: "3", bitWidth: "1", description: "Half-duplex selection", name: HDSEL }
                  { bitOffset: "2", bitWidth: "1", description: "IrDA low-power", name: IRLP }
                  { bitOffset: "1", bitWidth: "1", description: "IrDA mode enable", name: IREN }
                  { bitOffset: "0", bitWidth: "1", description: "Error interrupt enable", name: EIE }
                ]
              }
              name: CTLR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "Guard time and prescaler
          register"
              displayName: GPR
              fields:
              {
                field: [
                  { bitOffset: "8", bitWidth: "8", description: "Guard time value", name: GT }
                  { bitOffset: "0", bitWidth: "8", description: "Prescaler value", name: PSC }
                ]
              }
              name: GPR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        _attrs:
        {
          derivedFrom: USART1
        }
        baseAddress: "0x40004400"
        interrupt:
        {
          description: "USART2 global interrupt"
          name: USART2
          value: "54"
        }
        name: USART2
      }
      {
        _attrs:
        {
          derivedFrom: USART1
        }
        baseAddress: "0x40004800"
        interrupt:
        {
          description: "USART3 global interrupt"
          name: USART3
          value: "55"
        }
        name: USART3
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40012400"
        description: "Analog to digital converter"
        groupName: ADC
        interrupt:
        {
          description: "ADC1 global interrupt"
          name: ADC
          value: "34"
        }
        name: ADC
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "status register"
              displayName: STATR
              fields:
              {
                field: [
                  { bitOffset: "4", bitWidth: "1", description: "Regular channel start flag", name: STRT }
                  { bitOffset: "3", bitWidth: "1", description: "Injected channel start
              flag", name: JSTRT }
                  { bitOffset: "2", bitWidth: "1", description: "Injected channel end of
              conversion", name: JEOC }
                  { bitOffset: "1", bitWidth: "1", description: "Regular channel end of
              conversion", name: EOC }
                  { bitOffset: "0", bitWidth: "1", description: "Analog watchdog flag", name: AWD }
                ]
              }
              name: STATR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "control register 1 and TKEY_V control register"
              displayName: CTLR1_TKEY_V_CTLR
              fields:
              {
                field: [
                  { bitOffset: "28", bitWidth: "1", description: "Touch key count cycle time base", name: CCSEL }
                  { bitOffset: "27", bitWidth: "1", description: "count conversion complete flag", name: TKIF }
                  { bitOffset: "26", bitWidth: "1", description: "count cycle selection", name: TKCPS }
                  { bitOffset: "25", bitWidth: "1", description: "count conversion complete interrupt enabled", name: TKIEN }
                  { bitOffset: "24", bitWidth: "1", description: "Touch key enable, including TKEY_F and
              TKEY_V", name: TKENABLE }
                  { bitOffset: "23", bitWidth: "1", description: "Analog watchdog enable on regular
              channels", name: AWDEN }
                  { bitOffset: "22", bitWidth: "1", description: "Analog watchdog enable on injected
              channels", name: JAWDEN }
                  { bitOffset: "16", bitWidth: "4", description: "Dual mode selection", name: DUALMOD }
                  { bitOffset: "13", bitWidth: "3", description: "Discontinuous mode channel
              count", name: DISCNUM }
                  { bitOffset: "12", bitWidth: "1", description: "Discontinuous mode on injected
              channels", name: JDISCEN }
                  { bitOffset: "11", bitWidth: "1", description: "Discontinuous mode on regular
              channels", name: DISCEN }
                  { bitOffset: "10", bitWidth: "1", description: "Automatic injected group
              conversion", name: JAUTO }
                  { bitOffset: "9", bitWidth: "1", description: "Enable the watchdog on a single channel
              in scan mode", name: AWDSGL }
                  { bitOffset: "8", bitWidth: "1", description: "Scan mode", name: SCAN }
                  { bitOffset: "7", bitWidth: "1", description: "Interrupt enable for injected
              channels", name: JEOCIE }
                  { bitOffset: "6", bitWidth: "1", description: "Analog watchdog interrupt
              enable", name: AWDIE }
                  { bitOffset: "5", bitWidth: "1", description: "Interrupt enable for EOC", name: EOCIE }
                  { bitOffset: "0", bitWidth: "5", description: "Analog watchdog channel select
              bits", name: AWDCH }
                ]
              }
              name: CTLR1_TKEY_V_CTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "control register 2"
              displayName: CTLR2
              fields:
              {
                field: [
                  { bitOffset: "23", bitWidth: "1", description: "Temperature sensor and VREFINT
              enable", name: TSVREFE }
                  { bitOffset: "22", bitWidth: "1", description: "Start conversion of regular
              channels", name: SWSTART }
                  { bitOffset: "21", bitWidth: "1", description: "Start conversion of injected
              channels", name: JSWSTART }
                  { bitOffset: "20", bitWidth: "1", description: "External trigger conversion mode for
              regular channels", name: EXTTRIG }
                  { bitOffset: "17", bitWidth: "3", description: "External event select for regular
              group", name: EXTSEL }
                  { bitOffset: "15", bitWidth: "1", description: "External trigger conversion mode for
              injected channels", name: JEXTTRIG }
                  { bitOffset: "12", bitWidth: "3", description: "External event select for injected
              group", name: JEXTSEL }
                  { bitOffset: "11", bitWidth: "1", description: "Data alignment", name: ALIGN }
                  { bitOffset: "8", bitWidth: "1", description: "Direct memory access mode", name: DMA }
                  { bitOffset: "3", bitWidth: "1", description: "Reset calibration", name: RSTCAL }
                  { bitOffset: "2", bitWidth: "1", description: "A/D calibration", name: CAL }
                  { bitOffset: "1", bitWidth: "1", description: "Continuous conversion", name: CONT }
                  { bitOffset: "0", bitWidth: "1", description: "A/D converter ON / OFF", name: ADON }
                ]
              }
              name: CTLR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "sample time register 1"
              displayName: SAMPTR1
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "3", description: "Channel 10 sample time
              selection", name: SMP10 }
                  { bitOffset: "3", bitWidth: "3", description: "Channel 11 sample time
              selection", name: SMP11 }
                  { bitOffset: "6", bitWidth: "3", description: "Channel 12 sample time
              selection", name: SMP12 }
                  { bitOffset: "9", bitWidth: "3", description: "Channel 13 sample time
              selection", name: SMP13 }
                  { bitOffset: "12", bitWidth: "3", description: "Channel 14 sample time
              selection", name: SMP14 }
                  { bitOffset: "15", bitWidth: "3", description: "Channel 15 sample time
              selection", name: SMP15 }
                  { bitOffset: "18", bitWidth: "3", description: "Channel 16 sample time
              selection", name: SMP16 }
                  { bitOffset: "21", bitWidth: "3", description: "Channel 17 sample time
              selection", name: SMP17 }
                ]
              }
              name: SAMPTR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "sample time register 2"
              displayName: SAMPTR2
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "3", description: "Channel 0 sample time
              selection", name: SMP0 }
                  { bitOffset: "3", bitWidth: "3", description: "Channel 1 sample time
              selection", name: SMP1 }
                  { bitOffset: "6", bitWidth: "3", description: "Channel 2 sample time
              selection", name: SMP2 }
                  { bitOffset: "9", bitWidth: "3", description: "Channel 3 sample time
              selection", name: SMP3 }
                  { bitOffset: "12", bitWidth: "3", description: "Channel 4 sample time
              selection", name: SMP4 }
                  { bitOffset: "15", bitWidth: "3", description: "Channel 5 sample time
              selection", name: SMP5 }
                  { bitOffset: "18", bitWidth: "3", description: "Channel 6 sample time
              selection", name: SMP6 }
                  { bitOffset: "21", bitWidth: "3", description: "Channel 7 sample time
              selection", name: SMP7 }
                  { bitOffset: "24", bitWidth: "3", description: "Channel 8 sample time
              selection", name: SMP8 }
                  { bitOffset: "27", bitWidth: "3", description: "Channel 9 sample time
              selection", name: SMP9 }
                ]
              }
              name: SAMPTR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "injected channel data offset register
          x"
              displayName: IOFR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Data offset for injected channel
              x"
                  name: JOFFSET1
                }
              }
              name: IOFR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "injected channel data offset register
          x"
              displayName: IOFR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Data offset for injected channel
              x"
                  name: JOFFSET2
                }
              }
              name: IOFR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "injected channel data offset register
          x"
              displayName: IOFR3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Data offset for injected channel
              x"
                  name: JOFFSET3
                }
              }
              name: IOFR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x20"
              description: "injected channel data offset register
          x"
              displayName: IOFR4
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Data offset for injected channel
              x"
                  name: JOFFSET4
                }
              }
              name: IOFR4
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x24"
              description: "watchdog higher threshold
          register"
              displayName: WDHTR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Analog watchdog higher
              threshold"
                  name: HT
                }
              }
              name: WDHTR
              resetValue: "0x00000FFF"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x28"
              description: "watchdog lower threshold
          register"
              displayName: WDLTR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "Analog watchdog lower
              threshold"
                  name: LT
                }
              }
              name: WDLTR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x2C"
              description: "regular sequence register 1"
              displayName: RSQR1
              fields:
              {
                field: [
                  { bitOffset: "20", bitWidth: "4", description: "Regular channel sequence
              length", name: L }
                  { bitOffset: "15", bitWidth: "5", description: "16th conversion in regular
              sequence", name: SQ16 }
                  { bitOffset: "10", bitWidth: "5", description: "15th conversion in regular
              sequence", name: SQ15 }
                  { bitOffset: "5", bitWidth: "5", description: "14th conversion in regular
              sequence", name: SQ14 }
                  { bitOffset: "0", bitWidth: "5", description: "13th conversion in regular
              sequence", name: SQ13 }
                ]
              }
              name: RSQR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x30"
              description: "regular sequence register 2"
              displayName: RSQR2
              fields:
              {
                field: [
                  { bitOffset: "25", bitWidth: "5", description: "12th conversion in regular
              sequence", name: SQ12 }
                  { bitOffset: "20", bitWidth: "5", description: "11th conversion in regular
              sequence", name: SQ11 }
                  { bitOffset: "15", bitWidth: "5", description: "10th conversion in regular
              sequence", name: SQ10 }
                  { bitOffset: "10", bitWidth: "5", description: "9th conversion in regular
              sequence", name: SQ9 }
                  { bitOffset: "5", bitWidth: "5", description: "8th conversion in regular
              sequence", name: SQ8 }
                  { bitOffset: "0", bitWidth: "5", description: "7th conversion in regular
              sequence", name: SQ7 }
                ]
              }
              name: RSQR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x34"
              description: "regular sequence register 3"
              displayName: RSQR3
              fields:
              {
                field: [
                  { bitOffset: "25", bitWidth: "5", description: "6th conversion in regular
              sequence", name: SQ6 }
                  { bitOffset: "20", bitWidth: "5", description: "5th conversion in regular
              sequence", name: SQ5 }
                  { bitOffset: "15", bitWidth: "5", description: "4th conversion in regular
              sequence", name: SQ4 }
                  { bitOffset: "10", bitWidth: "5", description: "3rd conversion in regular
              sequence", name: SQ3 }
                  { bitOffset: "5", bitWidth: "5", description: "2nd conversion in regular
              sequence", name: SQ2 }
                  { bitOffset: "0", bitWidth: "5", description: "1st conversion in regular sequence_conversion count conversion channel selection", name: SQ1_CHSEL }
                ]
              }
              name: RSQR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x38"
              description: "injected sequence register"
              displayName: ISQR
              fields:
              {
                field: [
                  { bitOffset: "20", bitWidth: "2", description: "Injected sequence length", name: JL }
                  { bitOffset: "15", bitWidth: "5", description: "4th conversion in injected
              sequence", name: JSQ4 }
                  { bitOffset: "10", bitWidth: "5", description: "3rd conversion in injected
              sequence", name: JSQ3 }
                  { bitOffset: "5", bitWidth: "5", description: "2nd conversion in injected
              sequence", name: JSQ2 }
                  { bitOffset: "0", bitWidth: "5", description: "1st conversion in injected
              sequence", name: JSQ1 }
                ]
              }
              name: ISQR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x3C"
              description: "injected data register x"
              displayName: IDATAR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Injected data"
                  name: JDATA
                }
              }
              name: IDATAR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x40"
              description: "injected data register x"
              displayName: IDATAR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Injected data"
                  name: JDATA
                }
              }
              name: IDATAR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x44"
              description: "injected data register x"
              displayName: IDATAR3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Injected data"
                  name: JDATA
                }
              }
              name: IDATAR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x48"
              description: "injected data register x"
              displayName: IDATAR4
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Injected data"
                  name: JDATA
                }
              }
              name: IDATAR4
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x4C"
              description: "regular data register"
              displayName: RDATAR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "14", description: "Regular data_count conversion value", name: DATA0_13_TKDR }
                  { bitOffset: "14", bitWidth: "1", description: "Regular data", name: DATA14 }
                  { bitOffset: "15", bitWidth: "1", description: "Regular data_current working state of TKEY_V", name: DATA15_TKSTA }
                ]
              }
              name: RDATAR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40007400"
        description: "Digital to analog converter"
        groupName: DAC
        name: DAC1
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Control register (DAC_CTLR)"
              displayName: CTLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "DAC channel1 enable", name: EN1 }
                  { bitOffset: "1", bitWidth: "1", description: "DAC channel1 output buffer
              disable", name: BOFF1 }
                  { bitOffset: "2", bitWidth: "1", description: "DAC channel1 trigger
              enable", name: TEN1 }
                  { bitOffset: "3", bitWidth: "3", description: "DAC channel1 trigger
              selection", name: TSEL1 }
                  { bitOffset: "6", bitWidth: "2", description: "DAC channel1 noise/triangle wave
              generation enable", name: WAVE1 }
                  { bitOffset: "8", bitWidth: "4", description: "DAC channel1 mask/amplitude
              selector", name: MAMP1 }
                  { bitOffset: "12", bitWidth: "1", description: "DAC channel1 DMA enable", name: DMAEN1 }
                  { bitOffset: "16", bitWidth: "1", description: "DAC channel2 enable", name: EN2 }
                  { bitOffset: "17", bitWidth: "1", description: "DAC channel2 output buffer
              disable", name: BOFF2 }
                  { bitOffset: "18", bitWidth: "1", description: "DAC channel2 trigger
              enable", name: TEN2 }
                  { bitOffset: "19", bitWidth: "3", description: "DAC channel2 trigger
              selection", name: TSEL2 }
                  { bitOffset: "22", bitWidth: "2", description: "DAC channel2 noise/triangle wave
              generation enable", name: WAVE2 }
                  { bitOffset: "24", bitWidth: "4", description: "DAC channel2 mask/amplitude
              selector", name: MAMP2 }
                  { bitOffset: "28", bitWidth: "1", description: "DAC channel2 DMA enable", name: DMAEN2 }
                ]
              }
              name: CTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x4"
              description: "DAC software trigger register
          (DAC_SWTR)"
              displayName: SWTR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "DAC channel1 software
              trigger", name: SWTRIG1 }
                  { bitOffset: "1", bitWidth: "1", description: "DAC channel2 software
              trigger", name: SWTRIG2 }
                ]
              }
              name: SWTR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x8"
              description: "DAC channel1 12-bit right-aligned data
          holding register(DAC_R12BDHR1)"
              displayName: R12BDHR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "DAC channel1 12-bit right-aligned
              data"
                  name: DACC1DHR
                }
              }
              name: R12BDHR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0xC"
              description: "DAC channel1 12-bit left aligned data
          holding register (DAC_L12BDHR1)"
              displayName: L12BDHR1
              fields:
              {
                field:
                {
                  bitOffset: "4"
                  bitWidth: "12"
                  description: "DAC channel1 12-bit left-aligned
              data"
                  name: DACC1DHR
                }
              }
              name: L12BDHR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "DAC channel2 12-bit right aligned data
          holding register (DAC_R12BDHR2)"
              displayName: R12BDHR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "DAC channel2 12-bit right-aligned
              data"
                  name: DACC2DHR
                }
              }
              name: R12BDHR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "DAC channel2 12-bit left aligned data
          holding register (DAC_L12BDHR2)"
              displayName: L12BDHR2
              fields:
              {
                field:
                {
                  bitOffset: "4"
                  bitWidth: "12"
                  description: "DAC channel2 12-bit left-aligned
              data"
                  name: DACC2DHR
                }
              }
              name: L12BDHR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x2C"
              description: "DAC channel1 data output register
          (DAC_DOR1)"
              displayName: DOR1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "DAC channel1 data output"
                  name: DACC1DOR
                }
              }
              name: DOR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x30"
              description: "DAC channel2 data output register
          (DAC_DOR2)"
              displayName: DOR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "12"
                  description: "DAC channel2 data output"
                  name: DACC2DOR
                }
              }
              name: DOR2
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0xE000D000"
        description: "Debug support"
        groupName: DBG
        name: DBG
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x00"
              description: DBGMCU_CR1
              displayName: CR1
              fields:
              {
                field: [
                  { bitOffset: "7", bitWidth: "1", description: TIM4_STOP, name: TIM4_STOP }
                  { bitOffset: "6", bitWidth: "1", description: TIM3_STOP, name: TIM3_STOP }
                  { bitOffset: "5", bitWidth: "1", description: TIM2_STOP, name: TIM2_STOP }
                  { bitOffset: "4", bitWidth: "1", description: TIM1_STOP, name: TIM1_STOP }
                  { bitOffset: "3", bitWidth: "1", description: I2C2_SMBUS_TIMEOUT, name: I2C2_SMBUS_TIMEOUT }
                  { bitOffset: "2", bitWidth: "1", description: I2C1_SMBUS_TIMEOUT, name: I2C1_SMBUS_TIMEOUT }
                  { bitOffset: "1", bitWidth: "1", description: WWDG_STOP, name: WWDG_STOP }
                  { bitOffset: "0", bitWidth: "1", description: IWDG_STOP, name: IWDG_STOP }
                ]
              }
              name: CR1
              resetValue: "0x00"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: DBGMCU_CR2
              displayName: CR2
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: DBG_SLEEP, name: SLEEP }
                  { bitOffset: "1", bitWidth: "1", description: DBG_STOP, name: STOP }
                  { bitOffset: "2", bitWidth: "1", description: DBG_STANDBY, name: STANDBY }
                ]
              }
              name: CR2
              resetValue: "0x0"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40023400"
        description: "USB register"
        groupName: USB
        interrupt:
        {
          description: USBFS_IRQHandler
          name: USBFS
          value: "59"
        }
        name: USBFS
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x00"
              description: "USB base control"
              fields:
              {
                field: [
                  { bitRange: "[0:0]", description: "DMA enable and DMA interrupt enable for USB", name: RB_UC_DMA_EN }
                  { bitRange: "[1:1]", description: "force clear FIFO and count of USB", name: RB_UC_CLR_ALL }
                  { bitRange: "[2:2]", description: "force reset USB SIE, need software clear", name: RB_UC_RST_SIE }
                  { bitRange: "[3:3]", description: "enable automatic responding busy for device mode or automatic pause for host mode during interrupt flag UIF_TRANSFER valid", name: RB_UC_INT_BUSY }
                  { bitRange: "[5:4]", description: "bit mask of USB system control", name: MASK_UC_SYS_CTRL }
                  { bitRange: "[6:6]", description: "enable USB low speed: 0=12Mbps, 1=1.5Mbps", name: RB_UC_LOW_SPEED }
                  { bitRange: "[7:7]", description: "enable USB host mode: 0=device mode, 1=host mode", name: RB_UC_HOST_MODE }
                ]
              }
              name: R8_USB_CTRL
              resetValue: "0x06"
              size: "0X08"
            }
            {
              addressOffset: "0x01"
              description: "USB device physical prot control"
              fields:
              {
                field: [
                  { access: read-write, bitRange: "[0:0]", description: "enable USB physical port I/O: 0=disable, 1=enable;enable USB port: 0=disable, 1=enable port, automatic disabled if USB device detached", name: RB_UD_PORT_EN__RB_UH_PORT_EN }
                  { access: read-write, bitRange: "[1:1]", description: "general purpose bit;control USB bus reset: 0=normal, 1=force bus reset", name: RB_UD_GP_BIT__RB_UH_BUS_RESET }
                  { access: read-write, bitRange: "[2:2]", description: "enable USB physical port low speed: 0=full speed, 1=low speed;enable USB port low speed: 0=full speed, 1=low speed", name: RB_UD_LOW_SPEED__RB_UH_LOW_SPEED }
                  { access: read-only, bitRange: "[4:4]", description: "ReadOnly: indicate current UDM pin level", name: RB_UD_DM_PIN__RB_UH_DM_PIN }
                  { access: read-only, bitRange: "[5:5]", description: "ReadOnly: indicate current UDP pin level", name: RB_UD_DP_PIN__RB_UH_DP_PIN }
                  { access: read-only, bitRange: "[7:7]", description: "disable USB UDP/UDM pulldown resistance: 0=enable pulldown, 1=disable", name: RB_UD_PD_DIS__RB_UH_PD_DIS }
                ]
              }
              name: R8_UDEV_CTRL__R8_UHOST_CTRL
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x02"
              description: "USB interrupt enable"
              fields:
              {
                field: [
                  { bitRange: "[0:0]", description: "enable interrupt for USB bus reset event for USB device mode;enable interrupt for USB device detected event for USB host mode", name: RB_UIE_BUS_RST__RB_UIE_DETECT }
                  { bitRange: "[1:1]", description: "enable interrupt for USB transfer completion", name: RB_UIE_TRANSFER }
                  { bitRange: "[2:2]", description: "enable interrupt for USB suspend or resume event", name: RB_UIE_SUSPEND }
                  { bitRange: "[3:3]", description: "enable interrupt for host SOF timer action for USB host mode", name: RB_UIE_HST_SOF }
                  { bitRange: "[4:4]", description: "enable interrupt for FIFO overflow", name: RB_UIE_FIFO_OV }
                  { bitRange: "[6:6]", description: "enable interrupt for NAK responded for USB device mode", name: RB_UIE_DEV_NAK }
                  { bitRange: "[7:7]", description: "enable interrupt for SOF received for USB device mode", name: RB_UIE_DEV_SOF }
                ]
              }
              name: R8_USB_INT_EN
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x03"
              description: "USB device address"
              fields:
              {
                field: [
                  { bitRange: "[6:0]", description: "bit mask for USB device address", name: MASK_USB_ADDR }
                  { bitRange: "[7:7]", description: "general purpose bit", name: RB_UDA_GP_BIT }
                ]
              }
              name: R8_USB_DEV_AD
              size: "8"
            }
            {
              access: read-only
              addressOffset: "0x05"
              description: "USB miscellaneous status"
              fields:
              {
                field: [
                  { bitRange: "[0:0]", description: "RO, indicate device attached status on USB host", name: RB_UMS_DEV_ATTACH }
                  { bitRange: "[1:1]", description: "RO, indicate UDM level saved at device attached to USB host", name: RB_UMS_DM_LEVEL }
                  { bitRange: "[2:2]", description: "RO, indicate USB suspend status", name: RB_UMS_SUSPEND }
                  { bitRange: "[3:3]", description: "RO, indicate USB bus reset status", name: RB_UMS_BUS_RESET }
                  { bitRange: "[4:4]", description: "RO, indicate USB receiving FIFO ready status (not empty)", name: RB_UMS_R_FIFO_RDY }
                  { bitRange: "[5:5]", description: "RO, indicate USB SIE free status", name: RB_UMS_SIE_FREE }
                  { bitRange: "[6:6]", description: "RO, indicate host SOF timer action status for USB host", name: RB_UMS_SOF_ACT }
                  { bitRange: "[7:7]", description: "RO, indicate host SOF timer presage status", name: RB_UMS_SOF_PRES }
                ]
              }
              name: R8_USB_MIS_ST
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x06"
              description: "USB interrupt flag"
              fields:
              {
                field: [
                  { bitRange: "[0:0]", description: "bus reset event interrupt flag for USB device mode, direct bit address clear or write 1 to clear;device detected event interrupt flag for USB host mode, direct bit address clear or write 1 to clear", name: RB_UIF_BUS_RST__RB_UIF_DETECT }
                  { bitRange: "[1:1]", description: "USB transfer completion interrupt flag, direct bit address clear or write 1 to clear", name: RB_UIF_TRANSFER }
                  { bitRange: "[2:2]", description: "USB suspend or resume event interrupt flag, direct bit address clear or write 1 to clear", name: RB_UIF_SUSPEND }
                  { bitRange: "[3:3]", description: "host SOF timer interrupt flag for USB host, direct bit address clear or write 1 to clear", name: RB_UIF_HST_SOF }
                  { bitRange: "[4:4]", description: "FIFO overflow interrupt flag for USB, direct bit address clear or write 1 to clear", name: RB_UIF_FIFO_OV }
                  { access: read-only, bitRange: "[5:5]", description: "RO, indicate USB SIE free status", name: RB_U_SIE_FREE }
                  { access: read-only, bitRange: "[6:6]", description: "RO, indicate current USB transfer toggle is OK", name: RB_U_TOG_OK }
                  { access: read-only, bitRange: "[7:7]", description: "RO, indicate current USB transfer is NAK received", name: RB_U_IS_NAK }
                ]
              }
              name: R8_USB_INT_FG
              size: "8"
            }
            {
              access: read-only
              addressOffset: "0x07"
              description: "USB interrupt status"
              fields:
              {
                field: [
                  { bitRange: "[3:0]", description: "RO, bit mask of current transfer handshake response for USB host mode: 0000=no response, time out from device, others=handshake response PID received;RO, bit mask of current transfer endpoint number for USB device mode", name: MASK_UIS_H_RES__MASK_UIS_ENDP }
                  { bitRange: "[5:4]", description: "RO, bit mask of current token PID code received for USB device mode", name: MASK_UIS_TOKEN }
                  { bitRange: "[6:6]", description: "RO, indicate current USB transfer toggle is OK", name: RB_UIS_TOG_OK }
                  { bitRange: "[7:7]", description: "RO, indicate current USB transfer is NAK received for USB device mode", name: RB_UIS_IS_NAK }
                ]
              }
              name: R8_USB_INT_ST
              size: "8"
            }
            { access: read-only, addressOffset: "0x08", description: "USB receiving length", name: R8_USB_RX_LEN, size: "8" }
            {
              access: read-write
              addressOffset: "0x0C"
              description: "endpoint 4/1 mode"
              fields:
              {
                field: [
                  { bitRange: "[2:2]", description: "enable USB endpoint 4 transmittal (IN)", name: RB_UEP4_TX_EN }
                  { bitRange: "[3:3]", description: "enable USB endpoint 4 receiving (OUT)", name: RB_UEP4_RX_EN }
                  { bitRange: "[4:4]", description: "buffer mode of USB endpoint 1", name: RB_UEP1_BUF_MOD }
                  { bitRange: "[6:6]", description: "enable USB endpoint 1 transmittal (IN)", name: RB_UEP1_TX_EN }
                  { bitRange: "[7:7]", description: "enable USB endpoint 1 receiving (OUT)", name: RB_UEP1_RX_EN }
                ]
              }
              name: R8_UEP4_1_MOD
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x0D"
              description: "endpoint 2/3 mode;host endpoint mode"
              fields:
              {
                field: [
                  { bitRange: "[0:0]", description: "buffer mode of USB endpoint 2;buffer mode of USB host IN endpoint", name: RB_UEP2_BUF_MOD__RB_UH_EP_RBUF_MOD }
                  { bitRange: "[2:2]", description: "enable USB endpoint 2 transmittal (IN)", name: RB_UEP2_TX_EN }
                  { bitRange: "[3:3]", description: "enable USB endpoint 2 receiving (OUT);enable USB host IN endpoint receiving", name: RB_UEP2_RX_EN__RB_UH_EP_RX_EN }
                  { bitRange: "[4:4]", description: "buffer mode of USB endpoint 3;buffer mode of USB host OUT endpoint", name: RB_UEP3_BUF_MOD__RB_UH_EP_TBUF_MOD }
                  { bitRange: "[6:6]", description: "enable USB endpoint 3 transmittal (IN);enable USB host OUT endpoint transmittal", name: RB_UEP3_TX_EN__RB_UH_EP_TX_EN }
                  { bitRange: "[7:7]", description: "enable USB endpoint 3 receiving (OUT)", name: RB_UEP3_RX_EN }
                ]
              }
              name: R8_UEP2_3_MOD__R8_UH_EP_MOD
              size: "8"
            }
            { access: read-write, addressOffset: "0x10", description: "endpoint 0 DMA buffer address", name: R16_UEP0_DMA, size: "16" }
            { access: read-write, addressOffset: "0x14", description: "endpoint 1 DMA buffer address", name: R16_UEP1_DMA, size: "16" }
            { access: read-write, addressOffset: "0x18", description: "endpoint 2 DMA buffer address;host rx endpoint buffer high address", name: R16_UEP2_DMA__R16_UH_RX_DMA, size: "16" }
            { access: read-write, addressOffset: "0x1C", description: "endpoint 3 DMA buffer address;host tx endpoint buffer high address", name: R16_UEP3_DMA__R16_UH_TX_DMA, size: "16" }
            { access: read-write, addressOffset: "0x20", description: "endpoint 0 transmittal length", name: R8_UEP0_T_LEN, size: "8" }
            {
              access: read-write
              addressOffset: "0x22"
              description: "endpoint 0 control"
              fields:
              {
                field: [
                  { bitRange: "[1:0]", description: "bit mask of handshake response type for USB endpoint X transmittal (IN)", name: MASK_UEP_T_RES }
                  { bitRange: "[3:2]", description: "bit mask of handshake response type for USB endpoint X receiving (OUT)", name: MASK_UEP_R_RES }
                  { bitRange: "[4:4]", description: "enable automatic toggle after successful transfer completion on endpoint 1/2/3: 0=manual toggle, 1=automatic toggle", name: RB_UEP_AUTO_TOG }
                  { bitRange: "[6:6]", description: "prepared data toggle flag of USB endpoint X transmittal (IN): 0=DATA0, 1=DATA1", name: RB_UEP_T_TOG }
                  { bitRange: "[7:7]", description: "expected data toggle flag of USB endpoint X receiving (OUT): 0=DATA0, 1=DATA1", name: RB_UEP_R_TOG }
                ]
              }
              name: R8_UEP0_CTRL
              size: "8"
            }
            { access: read-write, addressOffset: "0x24", description: "endpoint 1 transmittal length", name: R8_UEP1_T_LEN, size: "8" }
            {
              access: read-write
              addressOffset: "0x26"
              description: "endpoint 1 control;host aux setup"
              fields:
              {
                field: [
                  { bitRange: "[1:0]", description: "bit mask of handshake response type for USB endpoint X transmittal (IN)", name: MASK_UEP_T_RES }
                  { bitRange: "[3:2]", description: "bit mask of handshake response type for USB endpoint X receiving (OUT)", name: MASK_UEP_R_RES }
                  { bitRange: "[4:4]", description: "enable automatic toggle after successful transfer completion on endpoint 1/2/3: 0=manual toggle, 1=automatic toggle", name: RB_UEP_AUTO_TOG }
                  { bitRange: "[6:6]", description: "prepared data toggle flag of USB endpoint X transmittal (IN): 0=DATA0, 1=DATA1;USB host automatic SOF enable", name: RB_UEP_T_TOG__RB_UH_SOF_EN }
                  { bitRange: "[7:7]", description: "expected data toggle flag of USB endpoint X receiving (OUT): 0=DATA0, 1=DATA1;RB_UH_PRE_PID_EN;USB host PRE PID enable for low speed device via hub", name: RB_UEP_R_TOG__RB_UH_PRE_PID_EN }
                ]
              }
              name: R8_UEP1_CTRL__R8_UH_SETUP
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x28"
              description: "endpoint 2 transmittal length;host endpoint and PID"
              fields:
              {
                field: [
                  { bitRange: "[3:0]", description: "bit mask of endpoint number for USB host transfer", name: MASK_UH_ENDP }
                  { bitRange: "[7:4]", description: "bit mask of token PID for USB host transfer", name: MASK_UH_TOKEN }
                ]
              }
              name: R8_UEP2_T_LEN__R8_UH_EP_PID
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x2A"
              description: "endpoint 2 control;host receiver endpoint control"
              fields:
              {
                field: [
                  { bitRange: "[1:0]", description: "bit mask of handshake response type for USB endpoint X transmittal (IN)", name: MASK_UEP_T_RES }
                  { bitRange: "[3:2]", description: "bit mask of handshake response type for USB endpoint X receiving (OUT)", name: MASK_UEP_R_RES }
                  { bitRange: "[4:4]", description: "enable automatic toggle after successful transfer completion on endpoint 1/2/3: 0=manual toggle, 1=automatic toggle;enable automatic toggle after successful transfer completion: 0=manual toggle, 1=automatic toggle", name: RB_UEP_AUTO_TOG__RB_UH_R_AUTO_TOG }
                  { bitRange: "[6:6]", description: "prepared data toggle flag of USB endpoint X transmittal (IN): 0=DATA0, 1=DATA1", name: RB_UEP_T_TOG }
                  { bitRange: "[7:7]", description: "expected data toggle flag of USB endpoint X receiving (OUT): 0=DATA0, 1=DATA1;expected data toggle flag of host receiving (IN): 0=DATA0, 1=DATA1", name: RB_UEP_R_TOG__RB_UH_R_TOG }
                ]
              }
              name: R8_UEP2_CTRL__R8_UH_RX_CTRL
              size: "8"
            }
            { access: read-write, addressOffset: "0x2C", description: "endpoint 3 transmittal length;host transmittal endpoint transmittal length", name: R8_UEP3_T_LEN__R8_UH_TX_LEN, size: "8" }
            {
              access: read-write
              addressOffset: "0x2E"
              description: "endpoint 3 control;host transmittal endpoint control"
              fields:
              {
                field: [
                  { bitRange: "[1:0]", description: "bit mask of handshake response type for USB endpoint X transmittal (IN)", name: MASK_UEP_T_RES }
                  { bitRange: "[3:2]", description: "bit mask of handshake response type for USB endpoint X receiving (OUT)", name: MASK_UEP_R_RES }
                  { bitRange: "[4:4]", description: "enable automatic toggle after successful transfer completion on endpoint 1/2/3: 0=manual toggle, 1=automatic toggle", name: RB_UEP_AUTO_TOG }
                  { bitRange: "[6:6]", description: "prepared data toggle flag of USB endpoint X transmittal (IN): 0=DATA0, 1=DATA1", name: RB_UEP_T_TOG }
                  { bitRange: "[7:7]", description: "expected data toggle flag of USB endpoint X receiving (OUT): 0=DATA0, 1=DATA1", name: RB_UEP_R_TOG }
                ]
              }
              name: R8_UEP3_CTRL__R8_UH_TX_CTRL
              size: "8"
            }
            { access: read-write, addressOffset: "0x30", description: "endpoint 4 transmittal length", name: R8_UEP4_T_LEN, size: "8" }
            {
              access: read-write
              addressOffset: "0x32"
              description: "endpoint 4 control"
              fields:
              {
                field: [
                  { bitRange: "[1:0]", description: "bit mask of handshake response type for USB endpoint X transmittal (IN)", name: MASK_UEP_T_RES }
                  { bitRange: "[3:2]", description: "bit mask of handshake response type for USB endpoint X receiving (OUT)", name: MASK_UEP_R_RES }
                  { bitRange: "[4:4]", description: "enable automatic toggle after successful transfer completion on endpoint 1/2/3: 0=manual toggle, 1=automatic toggle", name: RB_UEP_AUTO_TOG }
                  { bitRange: "[6:6]", description: "prepared data toggle flag of USB endpoint X transmittal (IN): 0=DATA0, 1=DATA1", name: RB_UEP_T_TOG }
                  { bitRange: "[7:7]", description: "expected data toggle flag of USB endpoint X receiving (OUT): 0=DATA0, 1=DATA1", name: RB_UEP_R_TOG }
                ]
              }
              name: R8_UEP4_CTRL
              size: "8"
            }
            {
              access: read-write
              addressOffset: "0x38"
              description: "USB type-C control"
              fields:
              {
                field: [
                  { bitRange: "[1:0]", description: "USB CC1 pullup resistance control", name: RB_UCC1_PU_EN }
                  { bitRange: "[2:2]", description: "USB CC1 5.1K pulldown resistance: 0=disable, 1=enable pulldown", name: RB_UCC1_PD_EN }
                  { bitRange: "[3:3]", description: "USB VBUS 10K pulldown resistance: 0=disable, 1=enable pullup", name: RB_VBUS_PD_EN }
                  { bitRange: "[5:4]", description: "USB CC2 pullup resistance control", name: RB_UCC2_PU_EN }
                  { bitRange: "[6:6]", description: "USB CC2 5.1K pulldown resistance: 0=disable, 1=enable pulldown", name: RB_UCC2_PD_EN }
                  { bitRange: "[7:7]", description: "USB general purpose bit", name: RB_UTCC_GP_BIT }
                ]
              }
              name: R8_USB_TYPE_C_CTRL
              size: "8"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40023000"
        description: "CRC calculation unit"
        groupName: CRC
        name: CRC
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x0"
              description: "Data register"
              displayName: DATAR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Data Register"
                  name: DATA
                }
              }
              name: DATAR
              resetValue: "0xFFFFFFFF"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x4"
              description: "Independent Data register"
              displayName: IDATAR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "8"
                  description: "Independent Data register"
                  name: IDATA
                }
              }
              name: IDATAR
              resetValue: "0x00"
              size: "0x8"
            }
            {
              access: write-only
              addressOffset: "0x8"
              description: "Control register"
              displayName: CTLR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "1"
                  description: "Reset bit"
                  name: RST
                }
              }
              name: CTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40022000"
        description: FLASH
        groupName: FLASH
        interrupt:
        {
          description: "Flash global interrupt"
          name: FLASH
          value: "20"
        }
        name: FLASH
        registers:
        {
          register: [
            {
              addressOffset: "0x0"
              description: "Flash access control register"
              displayName: ACTLR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "3", description: Latency, name: LATENCY }
                  { access: read-write, bitOffset: "4", bitWidth: "1", description: "Prefetch buffer enable", name: PRFTBE }
                  { access: read-only, bitOffset: "5", bitWidth: "1", description: "Prefetch buffer status", name: PRFTBS }
                ]
              }
              name: ACTLR
              resetValue: "0x00000030"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x4"
              description: "Flash key register"
              displayName: KEYR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "FPEC key"
                  name: KEYR
                }
              }
              name: KEYR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x8"
              description: "Flash option key register"
              displayName: OBKEYR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Option byte key"
                  name: OBKEYR
                }
              }
              name: OBKEYR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0xC"
              description: "Status register"
              displayName: STATR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "5", bitWidth: "1", description: "End of operation", name: EOP }
                  { access: read-write, bitOffset: "4", bitWidth: "1", description: "Write protection error", name: WRPRTERR }
                  { access: read-write, bitOffset: "2", bitWidth: "1", description: "Programming error", name: PGERR }
                  { access: read-only, bitOffset: "0", bitWidth: "1", description: Busy, name: BSY }
                ]
              }
              name: STATR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "Control register"
              displayName: CTLR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: Programming, name: PG }
                  { bitOffset: "1", bitWidth: "1", description: "Page Erase", name: PER }
                  { bitOffset: "2", bitWidth: "1", description: "Mass Erase", name: MER }
                  { bitOffset: "4", bitWidth: "1", description: "Option byte programming", name: OBPG }
                  { bitOffset: "5", bitWidth: "1", description: "Option byte erase", name: OBER }
                  { bitOffset: "6", bitWidth: "1", description: Start, name: STRT }
                  { bitOffset: "7", bitWidth: "1", description: Lock, name: LOCK }
                  { bitOffset: "9", bitWidth: "1", description: "Option bytes write enable", name: OPTWRE }
                  { bitOffset: "10", bitWidth: "1", description: "Error interrupt enable", name: ERRIE }
                  { bitOffset: "12", bitWidth: "1", description: "End of operation interrupt
              enable", name: EOPIE }
                  { bitOffset: "15", bitWidth: "1", description: "FAST programming lock", name: FLOCK }
                  { bitOffset: "16", bitWidth: "1", description: "execute fast programming", name: FTPG }
                  { bitOffset: "17", bitWidth: "1", description: "execute fast 128byte erase", name: FTER }
                  { bitOffset: "18", bitWidth: "1", description: "execute data load inner buffer", name: BUFLOAD }
                  { bitOffset: "19", bitWidth: "1", description: "execute inner buffer reset", name: BUFRST }
                ]
              }
              name: CTLR
              resetValue: "0x00000080"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x14"
              description: "Flash address register"
              displayName: ADDR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Flash Address"
                  name: FAR
                }
              }
              name: ADDR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x1C"
              description: "Option byte register"
              displayName: OBR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Option byte error", name: OPTERR }
                  { bitOffset: "1", bitWidth: "1", description: "Read protection", name: RDPRT }
                  { bitOffset: "2", bitWidth: "1", description: IWDG_SW, name: IWDGSW }
                  { bitOffset: "3", bitWidth: "1", description: nRST_STOP, name: STOPRST }
                  { bitOffset: "4", bitWidth: "1", description: nRST_STDBY, name: STANDYRST }
                  { bitOffset: "5", bitWidth: "1", description: "USBD compatible speed mode configure", name: USBDMODE }
                  { bitOffset: "6", bitWidth: "1", description: "USBD compatible inner pull up resistance configure", name: USBDPU }
                  { bitOffset: "7", bitWidth: "1", description: "Power on reset time", name: PORCTR }
                ]
              }
              name: OBR
              resetValue: "0x03FFFFFC"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x20"
              description: "Write protection register"
              displayName: WPR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Write protect"
                  name: WRP
                }
              }
              name: WPR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: write-only
              addressOffset: "0x24"
              description: "Extension key register"
              displayName: MODEKEYR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "high speed write /erase mode ENABLE"
                  name: MODEKEYR
                }
              }
              name: MODEKEYR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x1100"
          usage: registers
        }
        baseAddress: "0xE000E000"
        description: "Programmable Fast Interrupt Controller"
        groupName: PFIC
        name: PFIC
        registers:
        {
          register: [
            {
              access: read-only
              addressOffset: "0x00"
              description: "Interrupt Status Register"
              displayName: ISR1
              fields:
              {
                field: [
                  { bitOffset: "2", bitWidth: "2", description: "Interrupt ID Status", name: INTENSTA2_3 }
                  { bitOffset: "12", bitWidth: "20", description: "Interrupt ID Status", name: INTENSTA12_31 }
                ]
              }
              name: ISR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x04"
              description: "Interrupt Status Register"
              displayName: ISR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: "Interrupt ID Status"
                  name: INTENSTA
                }
              }
              name: ISR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x20"
              description: "Interrupt Pending Register"
              displayName: IPR1
              fields:
              {
                field: [
                  { bitOffset: "2", bitWidth: "2", description: PENDSTA, name: PENDSTA2_3 }
                  { bitOffset: "12", bitWidth: "20", description: PENDSTA, name: PENDSTA12_31 }
                ]
              }
              name: IPR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x24"
              description: "Interrupt Pending Register"
              displayName: IPR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: PENDSTA
                  name: PENDSTA
                }
              }
              name: IPR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x40"
              description: "Interrupt Priority
          Register"
              displayName: ITHRESDR
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "8"
                  description: THRESHOLD
                  name: THRESHOLD
                }
              }
              name: ITHRESDR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x44"
              description: "Interrupt Fast Address
          Register"
              displayName: FIBADDRR
              fields:
              {
                field:
                {
                  bitOffset: "28"
                  bitWidth: "4"
                  description: BASEADDR
                  name: BASEADDR
                }
              }
              name: FIBADDRR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x48"
              description: "Interrupt Config Register"
              displayName: CFGR
              fields:
              {
                field: [
                  { access: read-write, bitOffset: "0", bitWidth: "1", description: HWSTKCTRL, name: HWSTKCTRL }
                  { access: read-write, bitOffset: "1", bitWidth: "1", description: NESTCTRL, name: NESTCTRL }
                  { access: write-only, bitOffset: "2", bitWidth: "1", description: NMISET, name: NMISET }
                  { access: write-only, bitOffset: "3", bitWidth: "1", description: NMIRESET, name: NMIRESET }
                  { access: write-only, bitOffset: "4", bitWidth: "1", description: EXCSET, name: EXCSET }
                  { access: write-only, bitOffset: "5", bitWidth: "1", description: EXCRESET, name: EXCRESET }
                  { access: write-only, bitOffset: "6", bitWidth: "1", description: PFICRSET, name: PFICRSET }
                  { access: write-only, bitOffset: "7", bitWidth: "1", description: SYSRESET, name: SYSRESET }
                  { access: write-only, bitOffset: "16", bitWidth: "16", description: KEYCODE, name: KEYCODE }
                ]
              }
              name: CFGR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x4C"
              description: "Interrupt Global Register"
              displayName: GISR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "8", description: NESTSTA, name: NESTSTA }
                  { bitOffset: "8", bitWidth: "1", description: GACTSTA, name: GACTSTA }
                  { bitOffset: "9", bitWidth: "1", description: GPENDSTA, name: GPENDSTA }
                ]
              }
              name: GISR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x60"
              description: "Interrupt 0 address
          Register"
              displayName: FIFOADDRR0
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "24", description: OFFADDR0, name: OFFADDR0 }
                  { bitOffset: "24", bitWidth: "8", description: IRQID0, name: IRQID0 }
                ]
              }
              name: FIFOADDRR0
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x64"
              description: "Interrupt 1 address
          Register"
              displayName: FIFOADDRR1
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "24", description: OFFADDR1, name: OFFADDR1 }
                  { bitOffset: "24", bitWidth: "8", description: IRQID1, name: IRQID1 }
                ]
              }
              name: FIFOADDRR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x68"
              description: "Interrupt 2 address
          Register"
              displayName: FIFOADDRR2
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "24", description: OFFADDR2, name: OFFADDR2 }
                  { bitOffset: "24", bitWidth: "8", description: IRQID2, name: IRQID2 }
                ]
              }
              name: FIFOADDRR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x6C"
              description: "Interrupt 3 address
          Register"
              displayName: FIFOADDRR3
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "24", description: OFFADDR3, name: OFFADDR3 }
                  { bitOffset: "24", bitWidth: "8", description: IRQID3, name: IRQID3 }
                ]
              }
              name: FIFOADDRR3
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x100"
              description: "Interrupt Setting Register"
              displayName: IENR1
              fields:
              {
                field:
                {
                  bitOffset: "12"
                  bitWidth: "20"
                  description: INTEN
                  name: INTEN
                }
              }
              name: IENR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x104"
              description: "Interrupt Setting Register"
              displayName: IENR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: INTEN
                  name: INTEN
                }
              }
              name: IENR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x180"
              description: "Interrupt Clear Register"
              displayName: IRER1
              fields:
              {
                field:
                {
                  bitOffset: "12"
                  bitWidth: "20"
                  description: INTRSET
                  name: INTRSET
                }
              }
              name: IRER1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x184"
              description: "Interrupt Clear Register"
              displayName: IRER2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: INTRSET
                  name: INTRSET
                }
              }
              name: IRER2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x200"
              description: "Interrupt Pending Register"
              displayName: IPSR1
              fields:
              {
                field: [
                  { bitOffset: "2", bitWidth: "2", description: PENDSET, name: PENDSET2_3 }
                  { bitOffset: "12", bitWidth: "20", description: PENDSET, name: PENDSET12_31 }
                ]
              }
              name: IPSR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x204"
              description: "Interrupt Pending Register"
              displayName: IPSR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: PENDSET
                  name: PENDSET
                }
              }
              name: IPSR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x280"
              description: "Interrupt Pending Clear Register"
              displayName: IPRR1
              fields:
              {
                field: [
                  { bitOffset: "2", bitWidth: "2", description: PENDRESET, name: PENDRESET2_3 }
                  { bitOffset: "12", bitWidth: "20", description: PENDRESET, name: PENDRESET12_31 }
                ]
              }
              name: IPRR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x284"
              description: "Interrupt Pending Clear Register"
              displayName: IPRR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: PENDRESET
                  name: PENDRESET
                }
              }
              name: IPRR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x300"
              description: "Interrupt ACTIVE Register"
              displayName: IACTR1
              fields:
              {
                field:
                {
                  bitOffset: "12"
                  bitWidth: "20"
                  description: IACTS
                  name: IACTS
                }
              }
              name: IACTR1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0x304"
              description: "Interrupt ACTIVE Register"
              displayName: IACTR2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "28"
                  description: IACTS
                  name: IACTS
                }
              }
              name: IACTR2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-write
              addressOffset: "0xD10"
              description: "System Control Register"
              displayName: SCTLR
              fields:
              {
                field: [
                  { bitOffset: "1", bitWidth: "1", description: SLEEPONEXIT, name: SLEEPONEXIT }
                  { bitOffset: "2", bitWidth: "1", description: SLEEPDEEP, name: SLEEPDEEP }
                  { bitOffset: "3", bitWidth: "1", description: WFITOWFE, name: WFITOWFE }
                  { bitOffset: "4", bitWidth: "1", description: SEVONPEND, name: SEVONPEND }
                  { bitOffset: "5", bitWidth: "1", description: SETEVENT, name: SETEVENT }
                ]
              }
              name: SCTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              addressOffset: "0x1000"
              description: "System counting Control Register"
              displayName: STK_CTLR
              fields:
              {
                field:
                {
                  access: read-write
                  bitOffset: "0"
                  bitWidth: "28"
                  description: STE
                  name: STE
                }
              }
              name: STK_CTLR
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x00"
          size: "0x400"
          usage: registers
        }
        baseAddress: "0x40005C00"
        description: "Universal serial bus full-speed device
      interface"
        groupName: USB
        interrupt:
        {
          description: "USB Device FS Wakeup through EXTI line
        interrupt"
          name: USB_FS_WKUP
          value: "58"
        }
        name: USBD
        registers:
        {
          register: [
            {
              access: read-write
              addressOffset: "0x00"
              description: "endpoint 0 register"
              displayName: EPR0
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR0
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x04"
              description: "endpoint 1 register"
              displayName: EPR1
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR1
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x08"
              description: "endpoint 2 register"
              displayName: EPR2
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR2
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x0C"
              description: "endpoint 3 register"
              displayName: EPR3
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR3
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x10"
              description: "endpoint 4 register"
              displayName: EPR4
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR4
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x14"
              description: "endpoint 5 register"
              displayName: EPR5
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR5
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x18"
              description: "endpoint 6 register"
              displayName: EP6R
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR6
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x1C"
              description: "endpoint 7 register"
              displayName: EPR7
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint address", name: EA }
                  { bitOffset: "4", bitWidth: "2", description: "Status bits, for transmission
              transfers", name: STAT_TX }
                  { bitOffset: "6", bitWidth: "1", description: "Data Toggle, for transmission
              transfers", name: DTOG_TX }
                  { bitOffset: "7", bitWidth: "1", description: "Correct Transfer for
              transmission", name: CTR_TX }
                  { bitOffset: "8", bitWidth: "1", description: "Endpoint kind", name: EP_KIND }
                  { bitOffset: "9", bitWidth: "2", description: "Endpoint type", name: EP_TYPE }
                  { bitOffset: "11", bitWidth: "1", description: "Setup transaction
              completed", name: SETUP }
                  { bitOffset: "12", bitWidth: "2", description: "Status bits, for reception
              transfers", name: STAT_RX }
                  { bitOffset: "14", bitWidth: "1", description: "Data Toggle, for reception
              transfers", name: DTOG_RX }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer for
              reception", name: CTR_RX }
                ]
              }
              name: EPR7
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x40"
              description: "control register"
              displayName: USB_CNTR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "1", description: "Force USB Reset", name: FRES }
                  { bitOffset: "1", bitWidth: "1", description: "Power down", name: PDWN }
                  { bitOffset: "2", bitWidth: "1", description: "Low-power mode", name: LPMODE }
                  { bitOffset: "3", bitWidth: "1", description: "Force suspend", name: FSUSP }
                  { bitOffset: "4", bitWidth: "1", description: "Resume request", name: RESUME }
                  { bitOffset: "8", bitWidth: "1", description: "Expected start of frame interrupt
              mask", name: ESOFM }
                  { bitOffset: "9", bitWidth: "1", description: "Start of frame interrupt
              mask", name: SOFM }
                  { bitOffset: "10", bitWidth: "1", description: "USB reset interrupt mask", name: RESETM }
                  { bitOffset: "11", bitWidth: "1", description: "Suspend mode interrupt
              mask", name: SUSPM }
                  { bitOffset: "12", bitWidth: "1", description: "Wakeup interrupt mask", name: WKUPM }
                  { bitOffset: "13", bitWidth: "1", description: "Error interrupt mask", name: ERRM }
                  { bitOffset: "14", bitWidth: "1", description: "Packet memory area over / underrun
              interrupt mask", name: PMAOVRM }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer interrupt
              mask", name: CTRM }
                ]
              }
              name: CNTR
              resetValue: "0x0003"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x44"
              description: "interrupt status register"
              displayName: ISTR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "4", description: "Endpoint Identifier", name: EP_ID }
                  { bitOffset: "4", bitWidth: "1", description: "Direction of transaction", name: DIR }
                  { bitOffset: "8", bitWidth: "1", description: "Expected start frame", name: ESOF }
                  { bitOffset: "9", bitWidth: "1", description: "start of frame", name: SOF }
                  { bitOffset: "10", bitWidth: "1", description: "reset request", name: RESET }
                  { bitOffset: "11", bitWidth: "1", description: "Suspend mode request", name: SUSP }
                  { bitOffset: "12", bitWidth: "1", description: Wakeup, name: WKUP }
                  { bitOffset: "13", bitWidth: "1", description: Error, name: ERR }
                  { bitOffset: "14", bitWidth: "1", description: "Packet memory area over /
              underrun", name: PMAOVR }
                  { bitOffset: "15", bitWidth: "1", description: "Correct transfer", name: CTR }
                ]
              }
              name: ISTR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x48"
              description: "frame number register"
              displayName: FNR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "11", description: "Frame number", name: FN }
                  { bitOffset: "11", bitWidth: "2", description: "Lost SOF", name: LSOF }
                  { bitOffset: "13", bitWidth: "1", description: Locked, name: LCK }
                  { bitOffset: "14", bitWidth: "1", description: "Receive data - line status", name: RXDM }
                  { bitOffset: "15", bitWidth: "1", description: "Receive data + line status", name: RXDP }
                ]
              }
              name: FNR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x4C"
              description: "device address"
              displayName: DADDR
              fields:
              {
                field: [
                  { bitOffset: "0", bitWidth: "7", description: "Device address", name: ADD }
                  { bitOffset: "7", bitWidth: "1", description: "Enable function", name: EF }
                ]
              }
              name: DADDR
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-write
              addressOffset: "0x50"
              description: "Buffer table address"
              displayName: BTABLE
              fields:
              {
                field:
                {
                  bitOffset: "3"
                  bitWidth: "13"
                  description: "Buffer table"
                  name: BTABLE
                }
              }
              name: BTABLE
              resetValue: "0x0000"
              size: "0x10"
            }
          ]
        }
      }
      {
        addressBlock:
        {
          offset: "0x0"
          size: "0x14"
          usage: registers
        }
        baseAddress: "0x1FFFF7E0"
        description: "Device electronic signature"
        groupName: ESIG
        name: ESIG
        registers:
        {
          register: [
            {
              access: read-only
              addressOffset: "0x0"
              description: "Flash capacity register"
              displayName: FLACAP
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "16"
                  description: "Flash size"
                  name: F_SIZE_15_0
                }
              }
              name: FLACAP
              resetValue: "0x0000"
              size: "0x10"
            }
            {
              access: read-only
              addressOffset: "0x8"
              description: "Unique identity 1"
              displayName: UNIID1
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Unique identity[31:0]"
                  name: U_ID
                }
              }
              name: UNIID1
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0xC"
              description: "Unique identity 2"
              displayName: UNIID2
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Unique identity[63:32]"
                  name: U_ID
                }
              }
              name: UNIID2
              resetValue: "0x00000000"
              size: "0x20"
            }
            {
              access: read-only
              addressOffset: "0x10"
              description: "Unique identity 3"
              displayName: UNIID3
              fields:
              {
                field:
                {
                  bitOffset: "0"
                  bitWidth: "32"
                  description: "Unique identity[95:64]"
                  name: U_ID
                }
              }
              name: UNIID3
              resetValue: "0x00000000"
              size: "0x20"
            }
          ]
        }
      }
    ]
  }
  resetMask: "0xFFFFFFFF"
  resetValue: "0x0"
  size: "0x20"
  vendor: "WCH Ltd."
  vendorID: WCH
  version: "1.1"
  width: "32"
}
